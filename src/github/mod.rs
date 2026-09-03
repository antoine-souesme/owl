//! Client GraphQL de GitHub.
//!
//! Un seul point d'entrée HTTP. Le client classe chaque réponse, et c'est ce
//! classement qui pilote le traitement des erreurs décrit en
//! `docs/specs/05-erreurs-et-tests.md`.

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
    #[error("Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler.")]
    Unauthorized,
    #[error("Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`.")]
    Forbidden,
    /// L'heure de reprise est portée par la variante et non par le message :
    /// composer « limite d'appels atteinte, reprise à 14 h 32 » est une
    /// décision d'affichage, donc le travail de `app`, à la spec 05.
    #[error("Limite d'appels atteinte.")]
    RateLimited { reset_at: Option<DateTime<Utc>> },
    #[error("GitHub a répondu {0}.")]
    Http(u16),
    #[error("Réponse illisible de GitHub.")]
    Malformed,
    /// Aucun détail de `reqwest` n'est repris : ses messages peuvent citer
    /// l'URL et les en-têtes, où voyage le jeton.
    #[error("Réseau injoignable.")]
    Transport,
    #[error("Pull request introuvable.")]
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
        let mut entetes = HeaderMap::new();

        let mut autorisation = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| GithubError::Unauthorized)?;
        // Marqué sensible : `reqwest` ne l'écrit pas dans ses traces.
        autorisation.set_sensitive(true);
        entetes.insert(AUTHORIZATION, autorisation);
        entetes.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("owl/", env!("CARGO_PKG_VERSION"))),
        );

        let http = reqwest::Client::builder()
            .default_headers(entetes)
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
        variables: serde_json::Value,
    ) -> Result<T, GithubError> {
        let reponse = self
            .http
            .post(&self.endpoint)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|_| GithubError::Transport)?;

        let statut = reponse.status();
        let limite = limite_d_appels(reponse.headers());
        let corps = reponse.text().await.map_err(|_| GithubError::Transport)?;

        if statut == StatusCode::UNAUTHORIZED {
            return Err(GithubError::Unauthorized);
        }
        if statut == StatusCode::FORBIDDEN || statut == StatusCode::TOO_MANY_REQUESTS {
            // Le solde à zéro tranche pour la limite primaire ; `retry-after`
            // pour la limite secondaire. Les deux surviennent avec les
            // en-têtes `x-ratelimit-*`, présents aussi sur un simple refus de
            // droits : leur seule présence ne dit rien.
            return Err(match limite {
                Some(reset_at) => GithubError::RateLimited { reset_at },
                // Sans en-tête, seul le corps distingue la limite secondaire
                // du refus de droits ; un 429, lui, ne sert qu'aux limites.
                None if statut == StatusCode::TOO_MANY_REQUESTS
                    || limite_secondaire_annoncee(&corps) =>
                {
                    GithubError::RateLimited { reset_at: None }
                }
                None => GithubError::Forbidden,
            });
        }
        if !statut.is_success() {
            return Err(GithubError::Http(statut.as_u16()));
        }

        let enveloppe: Envelope<T> =
            serde_json::from_str(&corps).map_err(|_| GithubError::Malformed)?;

        if let Some(erreurs) = enveloppe.errors.filter(|liste| !liste.is_empty()) {
            let message = erreurs
                .into_iter()
                .map(|erreur| erreur.message)
                .collect::<Vec<_>>()
                .join(" · ");
            return Err(GithubError::Api(message));
        }

        enveloppe.data.ok_or(GithubError::Malformed)
    }

    /// Ramène les pull requests correspondant aux filtres, avec le solde
    /// d'appels lu au passage.
    ///
    /// Un solde à zéro n'est pas une erreur : les données de cette réponse
    /// sont bonnes. La suspension du rafraîchissement qu'il déclenche
    /// appartient à `05-erreurs-et-tests.md`.
    pub async fn fetch_pull_requests(
        &self,
        query: &str,
        page_size: u16,
    ) -> Result<ListPage, GithubError> {
        let variables = json!({ "q": query, "n": page_size });
        let donnees: dto::ListData = self.execute(queries::LIST, variables).await?;
        Ok(donnees.to_list_page())
    }

    /// Détail d'une seule pull request, lancé à l'ouverture de la vue détail.
    ///
    /// Le résumé déjà affiché est repris tel quel : la requête de détail ne
    /// renvoie aucun de ses champs. Elle apporte en revanche l'identifiant
    /// GraphQL, nécessaire à la fusion.
    pub async fn fetch_detail(&self, summary: &PrSummary) -> Result<PrDetail, GithubError> {
        let variables = json!({
            "owner": summary.key.owner(),
            "name": summary.key.name(),
            "number": summary.key.number,
        });
        let donnees: dto::DetailData = self.execute(queries::DETAIL, variables).await?;
        donnees
            .repository
            .and_then(|depot| depot.pull_request)
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
        let identifiant = match node_id {
            Some(valeur) => valeur,
            None => self.fetch_detail(summary).await?.node_id,
        };
        let variables = json!({ "id": identifiant, "method": methode_graphql(method) });
        // La réponse n'est pas modélisée : seule compte la distinction entre
        // succès et erreur, que `execute` a déjà faite.
        let _: serde_json::Value = self.execute(queries::MERGE, variables).await?;
        Ok(())
    }
}

/// Nom de la méthode dans le vocabulaire de GitHub. La traduction est ici et
/// nulle part ailleurs : `model` ne connaît pas ces mots.
fn methode_graphql(method: MergeMethod) -> &'static str {
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
fn limite_d_appels(entetes: &HeaderMap) -> Option<Option<DateTime<Utc>>> {
    if solde_epuise(entetes) {
        return Some(reset_at(entetes));
    }
    if let Some(delai) = retry_after(entetes) {
        return Some(Some(Utc::now() + chrono::Duration::seconds(delai)));
    }
    None
}

/// Vrai quand le corps de la réponse annonce une limite secondaire. GitHub y
/// écrit une phrase reconnaissable, seul indice quand les en-têtes de reprise
/// manquent. Classer ce refus en manque de droits ferait échouer le démarrage
/// sur un faux diagnostic.
fn limite_secondaire_annoncee(corps: &str) -> bool {
    let minuscules = corps.to_lowercase();
    ["secondary rate limit", "abuse detection mechanism"]
        .iter()
        .any(|marqueur| minuscules.contains(marqueur))
}

/// Vrai quand le solde de la limite primaire est explicitement à zéro.
fn solde_epuise(entetes: &HeaderMap) -> bool {
    entetes
        .get(REMAINING_HEADER)
        .and_then(|valeur| valeur.to_str().ok())
        .and_then(|brut| brut.trim().parse::<u64>().ok())
        == Some(0)
}

/// Heure de réinitialisation portée par l'en-tête de limite primaire, en
/// secondes depuis l'époque.
fn reset_at(entetes: &HeaderMap) -> Option<DateTime<Utc>> {
    let brut = entetes.get(RESET_HEADER)?.to_str().ok()?;
    let secondes: i64 = brut.trim().parse().ok()?;
    Utc.timestamp_opt(secondes, 0).single()
}

/// Délai, en secondes, avant de pouvoir réessayer une limite secondaire.
fn retry_after(entetes: &HeaderMap) -> Option<i64> {
    let brut = entetes.get(RETRY_AFTER_HEADER)?.to_str().ok()?;
    brut.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPONSE: &str = include_str!("../../tests/fixtures/list.json");

    /// Sert une seule réponse HTTP figée et rend l'adresse à viser.
    ///
    /// Un vrai serveur local plutôt qu'un client simulé : c'est le classement
    /// des réponses — code, en-têtes, corps — qui est testé ici, donc il faut
    /// que `reqwest` fasse réellement le trajet.
    async fn serveur(statut: &str, entetes: &[(&str, &str)], corps: &str) -> String {
        let ecoute = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("un port libre doit être disponible");
        let adresse = ecoute.local_addr().expect("adresse locale");

        let mut reponse = format!(
            "HTTP/1.1 {statut}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            corps.len()
        );
        for (nom, valeur) in entetes {
            reponse.push_str(&format!("{nom}: {valeur}\r\n"));
        }
        reponse.push_str("\r\n");
        reponse.push_str(corps);

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut flux, _) = ecoute.accept().await.expect("connexion acceptée");
            let mut tampon = [0u8; 8192];
            // La requête est lue puis ignorée : seule la réponse compte.
            let _ = flux.read(&mut tampon).await;
            let _ = flux.write_all(reponse.as_bytes()).await;
            let _ = flux.flush().await;
        });

        format!("http://{adresse}/graphql")
    }

    /// Sert plusieurs réponses HTTP figées d'affilée, une par connexion
    /// acceptée, dans l'ordre donné. Rend l'adresse à viser et le corps de
    /// chaque requête reçue, dans l'ordre où elles sont arrivées — c'est ce
    /// qui permet de vérifier qu'un appel enchaîne bien deux requêtes, et
    /// dans quel ordre.
    async fn serveur_enchaine(
        corps: &[&str],
    ) -> (String, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let ecoute = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("un port libre doit être disponible");
        let adresse = ecoute.local_addr().expect("adresse locale");

        let reponses: Vec<String> = corps
            .iter()
            .map(|corps| {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    corps.len(),
                    corps
                )
            })
            .collect();

        let (emetteur, recepteur) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            for reponse in reponses {
                let (mut flux, _) = ecoute.accept().await.expect("connexion acceptée");
                let mut tampon = [0u8; 8192];
                let lu = flux.read(&mut tampon).await.unwrap_or(0);
                let _ = emetteur.send(String::from_utf8_lossy(&tampon[..lu]).into_owned());
                let _ = flux.write_all(reponse.as_bytes()).await;
                let _ = flux.flush().await;
            }
        });

        (format!("http://{adresse}/graphql"), recepteur)
    }

    async fn appel(
        statut: &str,
        entetes: &[(&str, &str)],
        corps: &str,
    ) -> Result<crate::model::ListPage, GithubError> {
        let point = serveur(statut, entetes, corps).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        client
            .fetch_pull_requests("is:pr author:@me sort:updated-desc", 50)
            .await
    }

    #[tokio::test]
    async fn une_reponse_reussie_donne_la_liste_traduite() {
        let page = appel("200 OK", &[], REPONSE).await.expect("succès attendu");
        assert_eq!(page.pull_requests.len(), 5);
        assert_eq!(page.pull_requests[0].key.number, 42);
        assert_eq!(page.rate_limit.expect("solde présent").remaining, 4987);
    }

    #[tokio::test]
    async fn un_tableau_errors_reprend_les_messages_tels_quels() {
        let corps = r#"{"data":null,"errors":[{"message":"Could not resolve to a Repository"},{"message":"Field 'foo' doesn't exist"}]}"#;
        let erreur = appel("200 OK", &[], corps)
            .await
            .expect_err("erreur attendue");
        assert_eq!(
            erreur.to_string(),
            "Could not resolve to a Repository · Field 'foo' doesn't exist"
        );
    }

    #[tokio::test]
    async fn une_reponse_401_est_un_jeton_refuse() {
        let erreur = appel("401 Unauthorized", &[], r#"{"message":"Bad credentials"}"#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Unauthorized));
        assert_eq!(
            erreur.to_string(),
            "Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler."
        );
    }

    #[tokio::test]
    async fn une_reponse_403_sans_aucun_en_tete_est_un_manque_de_droits() {
        let erreur = appel(
            "403 Forbidden",
            &[],
            r#"{"message":"Resource not accessible"}"#,
        )
        .await
        .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Forbidden));
        assert_eq!(
            erreur.to_string(),
            "Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`."
        );
    }

    #[tokio::test]
    async fn une_reponse_403_avec_en_tetes_de_limite_non_atteinte_est_un_manque_de_droits() {
        // Cas réel de GitHub : un refus de droits porte quand même les
        // en-têtes `x-ratelimit-*`, avec un solde non nul.
        let erreur = appel(
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
        assert!(matches!(erreur, GithubError::Forbidden));
        assert_eq!(
            erreur.to_string(),
            "Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`."
        );
    }

    #[tokio::test]
    async fn une_reponse_403_de_limite_secondaire_sans_en_tete_est_une_limite_d_appels() {
        // Sans `retry-after` ni solde à zéro, seul le corps dit que le refus
        // est temporaire. Le classer en manque de droits ferait échouer le
        // démarrage sur un faux diagnostic.
        let erreur = appel(
            "403 Forbidden",
            &[],
            r#"{"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again."}"#,
        )
        .await
        .expect_err("erreur attendue");
        match erreur {
            GithubError::RateLimited { reset_at } => assert!(reset_at.is_none()),
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[tokio::test]
    async fn une_reponse_429_sans_en_tete_est_une_limite_d_appels() {
        let erreur = appel(
            "429 Too Many Requests",
            &[],
            r#"{"message":"Too many requests"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match erreur {
            GithubError::RateLimited { reset_at } => assert!(reset_at.is_none()),
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[tokio::test]
    async fn une_reponse_403_avec_solde_epuise_est_une_limite_d_appels() {
        // 1 788 084 720 = 2026-08-30T10:12:00Z
        let erreur = appel(
            "403 Forbidden",
            &[
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", "1788084720"),
            ],
            r#"{"message":"API rate limit exceeded"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match erreur {
            GithubError::RateLimited { reset_at } => assert_eq!(
                reset_at.expect("heure de reprise").to_rfc3339(),
                "2026-08-30T10:12:00+00:00"
            ),
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[tokio::test]
    async fn une_reponse_403_avec_retry_after_est_une_limite_d_appels_secondaire() {
        let avant = Utc::now();
        let erreur = appel(
            "403 Forbidden",
            &[("retry-after", "30")],
            r#"{"message":"You have exceeded a secondary rate limit"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match erreur {
            GithubError::RateLimited { reset_at } => {
                assert!(reset_at.expect("heure de reprise") > avant);
            }
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[tokio::test]
    async fn un_corps_tronque_est_une_reponse_illisible() {
        let erreur = appel("200 OK", &[], r#"{"data":{"search":{"nodes":["#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Malformed));
    }

    #[tokio::test]
    async fn une_reponse_5xx_rapporte_son_code() {
        let erreur = appel("502 Bad Gateway", &[], "")
            .await
            .expect_err("erreur attendue");
        assert_eq!(erreur.to_string(), "GitHub a répondu 502.");
    }

    #[tokio::test]
    async fn des_donnees_nulles_sans_erreur_sont_une_reponse_illisible() {
        let erreur = appel("200 OK", &[], r#"{"data":null}"#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Malformed));
    }

    #[tokio::test]
    async fn une_liste_sans_solde_d_appels_ne_porte_aucun_solde() {
        let corps = r#"{"data":{"search":{"nodes":[]}}}"#;
        let page = appel("200 OK", &[], corps).await.expect("succès attendu");
        assert!(page.rate_limit.is_none());
    }

    #[tokio::test]
    async fn le_client_ramene_le_detail_d_une_pull_request() {
        const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");
        let point = serveur("200 OK", &[], DETAIL).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        let resume = crate::model::PrSummary {
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
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: crate::model::RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            },
        };

        let detail = client.fetch_detail(&resume).await.expect("succès attendu");
        assert_eq!(detail.node_id, "PR_kwDOABCD12345");
        assert_eq!(detail.checks.len(), 5);
        assert_eq!(detail.summary, resume);
    }

    #[tokio::test]
    async fn une_pull_request_absente_de_la_reponse_est_introuvable() {
        let point = serveur("200 OK", &[], r#"{"data":{"repository":null}}"#).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        let resume = crate::model::PrSummary {
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
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: crate::model::RepoMergeRules {
                squash: true,
                merge: true,
                rebase: true,
                delete_branch_on_merge: false,
            },
        };
        let erreur = client
            .fetch_detail(&resume)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::NotFound));
    }

    /// Résumé minimal pour viser la mutation : seule la clé est lue quand
    /// l'identifiant GraphQL est déjà connu.
    fn resume_de_test() -> PrSummary {
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
    async fn une_fusion_reussie_ne_rend_rien() {
        let corps =
            r#"{"data":{"mergePullRequest":{"pullRequest":{"number":142,"state":"MERGED"}}}}"#;
        let adresse = serveur("200 OK", &[], corps).await;
        let client = Client::with_endpoint("jeton", &adresse).expect("client construit");

        let resultat = client
            .merge_pull_request(
                &resume_de_test(),
                Some("PR_identifiant".to_string()),
                MergeMethod::Squash,
            )
            .await;

        assert!(resultat.is_ok(), "{resultat:?}");
    }

    #[tokio::test]
    async fn une_fusion_refusee_rend_le_message_de_github_tel_quel() {
        let corps = r#"{"data":null,"errors":[{"message":"At least 1 approving review is required by reviewers with write access."}]}"#;
        let adresse = serveur("200 OK", &[], corps).await;
        let client = Client::with_endpoint("jeton", &adresse).expect("client construit");

        let erreur = client
            .merge_pull_request(
                &resume_de_test(),
                Some("PR_identifiant".to_string()),
                MergeMethod::Squash,
            )
            .await
            .expect_err("la mutation doit échouer");

        assert_eq!(
            erreur.to_string(),
            "At least 1 approving review is required by reviewers with write access."
        );
    }

    #[tokio::test]
    async fn sans_identifiant_le_detail_est_demande_avant_la_mutation() {
        const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");
        const MUTATION: &str =
            r#"{"data":{"mergePullRequest":{"pullRequest":{"number":142,"state":"MERGED"}}}}"#;
        let (adresse, mut requetes) = serveur_enchaine(&[DETAIL, MUTATION]).await;
        let client = Client::with_endpoint("jeton", &adresse).expect("client construit");

        let resultat = client
            .merge_pull_request(&resume_de_test(), None, MergeMethod::Squash)
            .await;

        assert!(resultat.is_ok(), "{resultat:?}");

        let premiere = requetes
            .recv()
            .await
            .expect("la requête de détail doit être envoyée");
        assert!(
            premiere.contains("query Detail"),
            "la première requête doit être le détail : {premiere}"
        );

        let seconde = requetes
            .recv()
            .await
            .expect("la requête de mutation doit être envoyée");
        assert!(
            seconde.contains("mutation Merge"),
            "la seconde requête doit être la mutation : {seconde}"
        );
        assert!(
            seconde.contains("PR_kwDOABCD12345"),
            "la mutation doit utiliser l'identifiant renvoyé par le détail : {seconde}"
        );
    }
}
