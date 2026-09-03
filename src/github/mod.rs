//! Client GraphQL de GitHub.
//!
//! Un seul point d'entrée HTTP. Le client classe chaque réponse, et c'est ce
//! classement qui pilote le traitement des erreurs décrit en
//! `docs/specs/05-errors-et-tests.md`.

pub mod dto;
pub mod queries;

use chrono::{DateTime, TimeZone, Utc};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

use crate::model::{ListPage, MergeMethod, PrDetail, PrSummary};

const ENDPOINT: &str = "https://api.github.com/graphql";

/// Heure de reprise de la limite primaire, en secondes depuis l'époque.
/// Présent sur la quasi-totalité des réponses de GitHub, refus de droits
/// compris : ce n'est pas lui qui distingue les deux cas.
const RESET_HEADER: &str = "x-ratelimit-reset";

/// Solde d'appels restant sur la limite primaire. C'est ce compteur, à zéro,
/// qui signale la limite atteinte.
const REMAINING_HEADER: &str = "x-ratelimit-remaining";

/// Délai d'attente, en secondes, que GitHub pose sur un refus de limite
/// secondaire (abus détecté, indépendant du compteur primaire).
const RETRY_AFTER_HEADER: &str = "retry-after";

#[derive(Debug, Error)]
pub enum GithubError {
    /// Réponse 200 accompagnée d'un tableau `errors`. Le message de GitHub
    /// est repris tel quel : il dit quoi faire mieux qu'un message maison.
    #[error("{0}")]
    Api(String),
    #[error("Token refused by GitHub. Run `gh auth login` to renew it.")]
    Unauthorized,
    #[error("The token lacks the required permissions. Check the `repo` scope.")]
    Forbidden,
    /// L'heure de reprise est portée par la variante et non par le message :
    /// composer « limite d'appels atteinte, reprise à 14 h 32 » est une
    /// décision d'affichage, donc le travail de `app`, à la spec 05.
    #[error("Rate limit reached.")]
    RateLimited { reset_at: Option<DateTime<Utc>> },
    #[error("GitHub responded {0}.")]
    Http(u16),
    #[error("Unreadable response from GitHub.")]
    Malformed,
    /// Aucun détail de `reqwest` n'est repris : ses messages peuvent citer
    /// l'URL et les en-têtes, où voyage le jeton.
    #[error("Network unreachable.")]
    Transport,
    #[error("Pull request not found.")]
    NotFound,
}

/// Enveloppe commune à toute réponse GraphQL.
#[derive(Deserialize)]
struct Envelope<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

/// Client HTTP prêt à l'emploi. Il porte le jeton dans ses en-têtes par
/// défaut : c'est le seul endroit du programme où le jeton est conservé, et il
/// n'en ressort jamais.
pub struct Client {
    http: reqwest::Client,
    endpoint: String,
}

impl Client {
    pub fn new(token: &str) -> Result<Self, GithubError> {
        Self::with_endpoint(token, ENDPOINT)
    }

    /// Point d'entrée réglable : les tests visent un serveur local.
    fn with_endpoint(token: &str, endpoint: &str) -> Result<Self, GithubError> {
        let mut headers = HeaderMap::new();

        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| GithubError::Unauthorized)?;
        // Marqué sensible : `reqwest` ne l'écrit pas dans ses traces.
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("owl/", env!("CARGO_PKG_VERSION"))),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            // Une connexion pendue ne renvoie jamais d'événement : sans
            // délai maximal, la barre d'état resterait bloquée sur son
            // message de chargement et les tâches de rafraîchissement
            // s'empileraient sans fin.
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|_| GithubError::Transport)?;

        Ok(Self {
            http,
            endpoint: endpoint.to_string(),
        })
    }

    /// Envoie un document et classe la réponse. Les quatre issues de la spec
    /// sont décidées ici, et nulle part ailleurs.
    async fn execute<T: DeserializeOwned>(
        &self,
        query: &str,
        variables_json: serde_json::Value,
    ) -> Result<T, GithubError> {
        let response = self
            .http
            .post(&self.endpoint)
            .json(&json!({ "query": query, "variables": variables_json }))
            .send()
            .await
            .map_err(|_| GithubError::Transport)?;

        let status = response.status();
        let limit = rate_limited(response.headers());
        let body = response.text().await.map_err(|_| GithubError::Transport)?;

        if status == StatusCode::UNAUTHORIZED {
            return Err(GithubError::Unauthorized);
        }
        if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
            // Le solde à zéro tranche pour la limite primaire ; `retry-after`
            // pour la limite secondaire. Les deux surviennent avec les
            // en-têtes `x-ratelimit-*`, présents aussi sur un simple refus de
            // droits : leur seule présence ne dit rien.
            return Err(match limit {
                Some(reset_at) => GithubError::RateLimited { reset_at },
                // Sans en-tête, seul le corps distingue la limite secondaire
                // du refus de droits ; un 429, lui, ne sert qu'aux limites.
                None if status == StatusCode::TOO_MANY_REQUESTS
                    || secondary_limit_announced(&body) =>
                {
                    GithubError::RateLimited { reset_at: None }
                }
                None => GithubError::Forbidden,
            });
        }
        if !status.is_success() {
            return Err(GithubError::Http(status.as_u16()));
        }

        let envelope: Envelope<T> =
            serde_json::from_str(&body).map_err(|_| GithubError::Malformed)?;

        if let Some(errors) = envelope.errors.filter(|list| !list.is_empty()) {
            let message = errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join(" · ");
            return Err(GithubError::Api(message));
        }

        envelope.data.ok_or(GithubError::Malformed)
    }

    /// Ramène les pull requests correspondant aux filtres, avec le solde
    /// d'appels lu au passage.
    ///
    /// Un solde à zéro n'est pas une erreur : les données de cette réponse
    /// sont bonnes. La suspension du rafraîchissement qu'il déclenche
    /// appartient à `05-errors-et-tests.md`.
    pub async fn fetch_pull_requests(
        &self,
        query: &str,
        page_size: u16,
    ) -> Result<ListPage, GithubError> {
        let variables_json = json!({ "q": query, "n": page_size });
        let data: dto::ListData = self.execute(queries::LIST, variables_json).await?;
        Ok(data.to_list_page())
    }

    /// Détail d'une seule pull request, lancé à l'ouverture de la vue détail.
    ///
    /// Le résumé déjà affiché est repris tel quel : la requête de détail ne
    /// renvoie aucun de ses champs. Elle apporte en revanche l'identifiant
    /// GraphQL, nécessaire à la fusion.
    pub async fn fetch_detail(&self, summary: &PrSummary) -> Result<PrDetail, GithubError> {
        let variables_json = json!({
            "owner": summary.key.owner(),
            "name": summary.key.name(),
            "number": summary.key.number,
        });
        let data: dto::DetailData = self.execute(queries::DETAIL, variables_json).await?;
        data.repository
            .and_then(|repo| repo.pull_request)
            .map(|pr| pr.to_detail(summary.clone()))
            .ok_or(GithubError::NotFound)
    }

    /// Fusionne une pull request avec la méthode donnée.
    ///
    /// L'identifiant GraphQL n'est pas dans la requête de liste. Quand
    /// l'appelant ne l'a pas — la vue détail n'a jamais été ouverte — il est
    /// récupéré ici par la requête de détail, puis la mutation enchaîne.
    /// L'enchaînement reste du réseau, donc il reste ici : `app` ne fait pas
    /// d'appel et n'a pas à connaître ce détour.
    ///
    /// Rien n'est rendu en cas de succès : `owl` ne lit pas la réponse de la
    /// mutation, il relance une requête de liste.
    pub async fn merge_pull_request(
        &self,
        summary: &PrSummary,
        node_id: Option<String>,
        method: MergeMethod,
    ) -> Result<(), GithubError> {
        let id = match node_id {
            Some(value) => value,
            None => self.fetch_detail(summary).await?.node_id,
        };
        let variables_json = json!({ "id": id, "method": graphql_method(method) });
        // La réponse n'est pas modélisée : seule compte la distinction entre
        // succès et erreur, que `execute` a déjà faite.
        let _: serde_json::Value = self.execute(queries::MERGE, variables_json).await?;
        Ok(())
    }
}

/// Nom de la méthode dans le vocabulaire de GitHub. La traduction est ici et
/// nulle part ailleurs : `model` ne connaît pas ces mots.
fn graphql_method(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Squash => "SQUASH",
        MergeMethod::Rebase => "REBASE",
        MergeMethod::Merge => "MERGE",
    }
}

/// Détecte une limite d'appels atteinte, primaire ou secondaire, et rend son
/// heure de reprise si GitHub la donne.
///
/// `Some(_)` signale la limite atteinte ; `None` laisse le classement à un
/// simple refus de droits. La primaire se lit sur un solde à zéro, la
/// secondaire sur `retry-after`, un délai en secondes converti ici en heure
/// absolue.
fn rate_limited(headers: &HeaderMap) -> Option<Option<DateTime<Utc>>> {
    if remaining_exhausted(headers) {
        return Some(reset_at(headers));
    }
    if let Some(delay) = retry_after(headers) {
        return Some(Some(Utc::now() + chrono::Duration::seconds(delay)));
    }
    None
}

/// Vrai quand le corps de la réponse annonce une limite secondaire. GitHub y
/// écrit une phrase reconnaissable, seul indice quand les en-têtes de reprise
/// manquent. Classer ce refus en manque de droits ferait échouer le démarrage
/// sur un faux diagnostic.
fn secondary_limit_announced(body: &str) -> bool {
    let minuscules = body.to_lowercase();
    ["secondary rate limit", "abuse detection mechanism"]
        .iter()
        .any(|marqueur| minuscules.contains(marqueur))
}

/// Vrai quand le solde de la limite primaire est explicitement à zéro.
fn remaining_exhausted(headers: &HeaderMap) -> bool {
    headers
        .get(REMAINING_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        == Some(0)
}

/// Heure de réinitialisation portée par l'en-tête de limite primaire, en
/// secondes depuis l'époque.
fn reset_at(headers: &HeaderMap) -> Option<DateTime<Utc>> {
    let raw = headers.get(RESET_HEADER)?.to_str().ok()?;
    let seconds: i64 = raw.trim().parse().ok()?;
    Utc.timestamp_opt(seconds, 0).single()
}

/// Délai, en secondes, avant de pouvoir réessayer une limite secondaire.
fn retry_after(headers: &HeaderMap) -> Option<i64> {
    let raw = headers.get(RETRY_AFTER_HEADER)?.to_str().ok()?;
    raw.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = include_str!("../../tests/fixtures/list.json");

    /// Sert une seule réponse HTTP figée et rend l'address à viser.
    ///
    /// Un vrai serveur local plutôt qu'un client simulé : c'est le classement
    /// des réponses — code, en-têtes, corps — qui est testé ici, donc il faut
    /// que `reqwest` fasse réellement le trajet.
    async fn server(status: &str, headers: &[(&str, &str)], body: &str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("un port libre doit être disponible");
        let address = listener.local_addr().expect("address locale");

        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        response.push_str(body);

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut stream, _) = listener.accept().await.expect("connexion acceptée");
            let mut buffer = [0u8; 8192];
            // La requête est lue puis ignorée : seule la réponse compte.
            let _ = stream.read(&mut buffer).await;
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        });

        format!("http://{address}/graphql")
    }

    /// Sert plusieurs réponses HTTP figées d'affilée, une par connexion
    /// acceptée, dans l'ordre donné. Rend l'address à viser et le corps de
    /// chaque requête reçue, dans l'ordre où elles sont arrivées — c'est ce
    /// qui permet de vérifier qu'un appel enchaîne bien deux requêtes, et
    /// dans quel ordre.
    async fn chained_server(
        body: &[&str],
    ) -> (String, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("un port libre doit être disponible");
        let address = listener.local_addr().expect("address locale");

        let responses: Vec<String> = body
            .iter()
            .map(|body| {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            })
            .collect();

        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            for response in responses {
                let (mut stream, _) = listener.accept().await.expect("connexion acceptée");
                let mut buffer = [0u8; 8192];
                let read_bytes = stream.read(&mut buffer).await.unwrap_or(0);
                let _ = sender.send(String::from_utf8_lossy(&buffer[..read_bytes]).into_owned());
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });

        (format!("http://{address}/graphql"), receiver)
    }

    async fn call(
        status: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<crate::model::ListPage, GithubError> {
        let endpoint = server(status, headers, body).await;
        let client = Client::with_endpoint("jeton-de-test", &endpoint).expect("client construit");
        client
            .fetch_pull_requests("is:pr author:@me sort:updated-desc", 50)
            .await
    }

    #[tokio::test]
    async fn a_successful_response_gives_the_translated_list() {
        let page = call("200 OK", &[], RESPONSE).await.expect("succès attendu");
        assert_eq!(page.pull_requests.len(), 5);
        assert_eq!(page.pull_requests[0].key.number, 42);
        assert_eq!(page.rate_limit.expect("solde présent").remaining, 4987);
    }

    #[tokio::test]
    async fn an_errors_array_repeats_the_messages_verbatim() {
        let body = r#"{"data":null,"errors":[{"message":"Could not resolve to a Repository"},{"message":"Field 'foo' doesn't exist"}]}"#;
        let error = call("200 OK", &[], body)
            .await
            .expect_err("erreur attendue");
        assert_eq!(
            error.to_string(),
            "Could not resolve to a Repository · Field 'foo' doesn't exist"
        );
    }

    #[tokio::test]
    async fn a_401_response_is_a_refused_token() {
        let error = call("401 Unauthorized", &[], r#"{"message":"Bad credentials"}"#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(error, GithubError::Unauthorized));
        assert_eq!(
            error.to_string(),
            "Token refused by GitHub. Run `gh auth login` to renew it."
        );
    }

    #[tokio::test]
    async fn a_403_without_any_header_is_missing_permissions() {
        let error = call(
            "403 Forbidden",
            &[],
            r#"{"message":"Resource not accessible"}"#,
        )
        .await
        .expect_err("erreur attendue");
        assert!(matches!(error, GithubError::Forbidden));
        assert_eq!(
            error.to_string(),
            "The token lacks the required permissions. Check the `repo` scope."
        );
    }

    #[tokio::test]
    async fn a_403_with_headers_showing_calls_left_is_missing_permissions() {
        // Cas réel de GitHub : un refus de droits porte quand même les
        // en-têtes `x-ratelimit-*`, avec un solde non nul.
        let error = call(
            "403 Forbidden",
            &[
                ("x-ratelimit-limit", "5000"),
                ("x-ratelimit-remaining", "4999"),
                ("x-ratelimit-reset", "1788348917"),
            ],
            r#"{"message":"Resource not accessible"}"#,
        )
        .await
        .expect_err("erreur attendue");
        assert!(matches!(error, GithubError::Forbidden));
        assert_eq!(
            error.to_string(),
            "The token lacks the required permissions. Check the `repo` scope."
        );
    }

    #[tokio::test]
    async fn a_403_secondary_limit_without_headers_is_a_rate_limit() {
        // Sans `retry-after` ni solde à zéro, seul le corps dit que le refus
        // est temporaire. Le classer en manque de droits ferait échouer le
        // démarrage sur un faux diagnostic.
        let error = call(
            "403 Forbidden",
            &[],
            r#"{"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again."}"#,
        )
        .await
        .expect_err("erreur attendue");
        match error {
            GithubError::RateLimited { reset_at } => assert!(reset_at.is_none()),
            other => panic!("erreur inattendue : {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_429_without_headers_is_a_rate_limit() {
        let error = call(
            "429 Too Many Requests",
            &[],
            r#"{"message":"Too many requests"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match error {
            GithubError::RateLimited { reset_at } => assert!(reset_at.is_none()),
            other => panic!("erreur inattendue : {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_403_with_an_exhausted_remaining_is_a_rate_limit() {
        // 1 788 084 720 = 2026-08-30T10:12:00Z
        let error = call(
            "403 Forbidden",
            &[
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", "1788084720"),
            ],
            r#"{"message":"API rate limit exceeded"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match error {
            GithubError::RateLimited { reset_at } => assert_eq!(
                reset_at.expect("heure de reprise").to_rfc3339(),
                "2026-08-30T10:12:00+00:00"
            ),
            other => panic!("erreur inattendue : {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_403_with_retry_after_is_a_secondary_rate_limit() {
        let avant = Utc::now();
        let error = call(
            "403 Forbidden",
            &[("retry-after", "30")],
            r#"{"message":"You have exceeded a secondary rate limit"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match error {
            GithubError::RateLimited { reset_at } => {
                assert!(reset_at.expect("heure de reprise") > avant);
            }
            other => panic!("erreur inattendue : {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_truncated_body_is_an_unreadable_response() {
        let error = call("200 OK", &[], r#"{"data":{"search":{"nodes":["#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(error, GithubError::Malformed));
    }

    #[tokio::test]
    async fn a_5xx_response_reports_its_code() {
        let error = call("502 Bad Gateway", &[], "")
            .await
            .expect_err("erreur attendue");
        assert_eq!(error.to_string(), "GitHub responded 502.");
    }

    #[tokio::test]
    async fn null_data_without_errors_is_an_unreadable_response() {
        let error = call("200 OK", &[], r#"{"data":null}"#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(error, GithubError::Malformed));
    }

    #[tokio::test]
    async fn a_list_without_a_rate_limit_carries_none() {
        let body = r#"{"data":{"search":{"nodes":[]}}}"#;
        let page = call("200 OK", &[], body).await.expect("succès attendu");
        assert!(page.rate_limit.is_none());
    }

    #[tokio::test]
    async fn the_client_fetches_the_detail_of_a_pull_request() {
        const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");
        let endpoint = server("200 OK", &[], DETAIL).await;
        let client = Client::with_endpoint("jeton-de-test", &endpoint).expect("client construit");
        let summary = crate::model::PrSummary {
            key: crate::model::PrKey {
                repo: "moi/owl".to_string(),
                number: 42,
            },
            title: "Ajoute la fenêtre de fusion".to_string(),
            author: "moi".to_string(),
            url: "https://github.com/moi/owl/pull/42".to_string(),
            is_draft: false,
            checks: crate::model::ChecksState::Success,
            review: crate::model::ReviewState::Approved,
            mergeable: crate::model::MergeableState::Mergeable,
            base_ref: "develop".to_string(),
            head_ref: "ma-branche".to_string(),
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: crate::model::RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            },
        };

        let detail = client.fetch_detail(&summary).await.expect("succès attendu");
        assert_eq!(detail.node_id, "PR_kwDOABCD12345");
        assert_eq!(detail.checks.len(), 5);
        assert_eq!(detail.summary, summary);
    }

    #[tokio::test]
    async fn a_pull_request_missing_from_the_response_is_not_found() {
        let endpoint = server("200 OK", &[], r#"{"data":{"repository":null}}"#).await;
        let client = Client::with_endpoint("jeton-de-test", &endpoint).expect("client construit");
        let summary = crate::model::PrSummary {
            key: crate::model::PrKey {
                repo: "moi/owl".to_string(),
                number: 1,
            },
            title: String::new(),
            author: "moi".to_string(),
            url: String::new(),
            is_draft: false,
            checks: crate::model::ChecksState::None,
            review: crate::model::ReviewState::None,
            mergeable: crate::model::MergeableState::Unknown,
            base_ref: "develop".to_string(),
            head_ref: "ma-branche".to_string(),
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: crate::model::RepoMergeRules {
                squash: true,
                merge: true,
                rebase: true,
                delete_branch_on_merge: false,
            },
        };
        let error = client
            .fetch_detail(&summary)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(error, GithubError::NotFound));
    }

    /// Résumé minimal pour viser la mutation : seule la clé est lue quand
    /// l'identifiant GraphQL est déjà connu.
    fn test_summary() -> PrSummary {
        use crate::model::{ChecksState, MergeableState, PrKey, RepoMergeRules, ReviewState};
        PrSummary {
            key: PrKey {
                repo: "moi/depot".to_string(),
                number: 142,
            },
            title: "Corrige la lecture des réglages".to_string(),
            author: "moi".to_string(),
            url: "https://github.com/moi/depot/pull/142".to_string(),
            is_draft: false,
            checks: ChecksState::Success,
            review: ReviewState::Approved,
            mergeable: MergeableState::Mergeable,
            base_ref: "develop".to_string(),
            head_ref: "ma-branche".to_string(),
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: RepoMergeRules {
                squash: true,
                merge: false,
                rebase: false,
                delete_branch_on_merge: true,
            },
        }
    }

    #[tokio::test]
    async fn a_successful_merge_returns_nothing() {
        let body =
            r#"{"data":{"mergePullRequest":{"pullRequest":{"number":142,"state":"MERGED"}}}}"#;
        let address = server("200 OK", &[], body).await;
        let client = Client::with_endpoint("jeton", &address).expect("client construit");

        let result = client
            .merge_pull_request(
                &test_summary(),
                Some("PR_identifiant".to_string()),
                MergeMethod::Squash,
            )
            .await;

        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn a_refused_merge_returns_the_github_message_verbatim() {
        let body = r#"{"data":null,"errors":[{"message":"At least 1 approving review is required by reviewers with write access."}]}"#;
        let address = server("200 OK", &[], body).await;
        let client = Client::with_endpoint("jeton", &address).expect("client construit");

        let error = client
            .merge_pull_request(
                &test_summary(),
                Some("PR_identifiant".to_string()),
                MergeMethod::Squash,
            )
            .await
            .expect_err("la mutation doit échouer");

        assert_eq!(
            error.to_string(),
            "At least 1 approving review is required by reviewers with write access."
        );
    }

    #[tokio::test]
    async fn without_the_id_the_detail_is_fetched_before_the_mutation() {
        const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");
        const MUTATION: &str =
            r#"{"data":{"mergePullRequest":{"pullRequest":{"number":142,"state":"MERGED"}}}}"#;
        let (address, mut queries) = chained_server(&[DETAIL, MUTATION]).await;
        let client = Client::with_endpoint("jeton", &address).expect("client construit");

        let result = client
            .merge_pull_request(&test_summary(), None, MergeMethod::Squash)
            .await;

        assert!(result.is_ok(), "{result:?}");

        let first_one = queries
            .recv()
            .await
            .expect("la requête de détail doit être envoyée");
        assert!(
            first_one.contains("query Detail"),
            "la première requête doit être le détail : {first_one}"
        );

        let second_one = queries
            .recv()
            .await
            .expect("la requête de mutation doit être envoyée");
        assert!(
            second_one.contains("mutation Merge"),
            "la seconde requête doit être la mutation : {second_one}"
        );
        assert!(
            second_one.contains("PR_kwDOABCD12345"),
            "la mutation doit utiliser l'identifiant renvoyé par le détail : {second_one}"
        );
    }
}
