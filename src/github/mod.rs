//! Client GraphQL de GitHub.
//!
//! Un seul point d'entrée HTTP. Le client classe chaque réponse, et c'est ce
//! classement qui pilote le traitement des erreurs décrit en
//! `docs/specs/05-erreurs-et-tests.md`.

// `fetch_detail` est appelée par la spec 03. Un attribut interne doit précéder
// tout élément du fichier, déclarations de modules comprises.
#![allow(dead_code)]

pub mod dto;
pub mod queries;

use chrono::{DateTime, TimeZone, Utc};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

use crate::model::{ListPage, PrDetail, PrSummary};

const ENDPOINT: &str = "https://api.github.com/graphql";

/// En-tête que GitHub renvoie avec un refus pour limite d'appels : c'est lui
/// qui distingue une limite atteinte d'un simple manque de droits.
const RESET_HEADER: &str = "x-ratelimit-reset";

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
    pub fn with_endpoint(token: &str, endpoint: &str) -> Result<Self, GithubError> {
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
        let reprise = reset_at(reponse.headers());
        let corps = reponse.text().await.map_err(|_| GithubError::Transport)?;

        if statut == StatusCode::UNAUTHORIZED {
            return Err(GithubError::Unauthorized);
        }
        if statut == StatusCode::FORBIDDEN || statut == StatusCode::TOO_MANY_REQUESTS {
            // C'est l'en-tête de réinitialisation qui tranche : sans lui, le
            // refus porte sur les droits, pas sur le nombre d'appels.
            return Err(match reprise {
                Some(_) => GithubError::RateLimited { reset_at: reprise },
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
        filters: &[String],
        page_size: u16,
    ) -> Result<ListPage, GithubError> {
        let variables = json!({ "q": search_query(filters), "n": page_size });
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
}

/// Heure de réinitialisation portée par l'en-tête de limite d'appels, en
/// secondes depuis l'époque.
fn reset_at(entetes: &HeaderMap) -> Option<DateTime<Utc>> {
    let brut = entetes.get(RESET_HEADER)?.to_str().ok()?;
    let secondes: i64 = brut.trim().parse().ok()?;
    Utc.timestamp_opt(secondes, 0).single()
}

/// Assemble la chaîne de recherche.
///
/// Bouchon jusqu'à `docs/specs/02-filtres.md` : les filtres sont déjà écrits
/// dans la syntaxe de GitHub, il suffit de les joindre. `filter::build_query`
/// prendra sa place et ajoutera `is:pr` et `sort:updated-desc`. Rien n'est
/// ajouté ici, pour ne pas dupliquer une règle qui appartient à la spec 02.
fn search_query(filters: &[String]) -> String {
    filters.join(" ")
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

    async fn appel(
        statut: &str,
        entetes: &[(&str, &str)],
        corps: &str,
    ) -> Result<crate::model::ListPage, GithubError> {
        let point = serveur(statut, entetes, corps).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        client
            .fetch_pull_requests(&["author:@me".to_string()], 50)
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
    async fn une_reponse_403_sans_en_tete_est_un_manque_de_droits() {
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
    async fn une_reponse_403_avec_en_tete_de_reinitialisation_est_une_limite_d_appels() {
        // 1 788 084 720 = 2026-08-30T10:12:00Z
        let erreur = appel(
            "403 Forbidden",
            &[("x-ratelimit-reset", "1788084720")],
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

    #[test]
    fn la_chaine_de_recherche_joint_les_filtres() {
        assert_eq!(
            search_query(&["author:@me".to_string(), "is:open".to_string()]),
            "author:@me is:open"
        );
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
}
