//! Forme du JSON écrit par les sous-commandes. Ce fichier est le contrat lu
//! par l'app de la barre des menus : tout changement de forme passe par une
//! hausse de `FORMAT_VERSION`.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::config::{Config, ConfigError};
use crate::github::GithubError;
use crate::model::{
    ChecksState, ListPage, MergeMethod, MergeState, MergeableState, PrDetail, PrSummary,
    ReviewState,
};
use crate::ports::{Listing, PortsError, Stopped};
use crate::token::TokenError;

/// Version du format. L'app refuse toute autre valeur.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
struct Envelope<T: Serialize> {
    version: u32,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

/// Erreur telle que l'app la lit. `message` est le texte exact que la TUI
/// afficherait ; `kind` permet à l'app de réagir sans analyser le texte.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<String>,
}

impl ErrorBody {
    fn new(kind: &'static str, message: String) -> Self {
        Self {
            kind,
            message,
            reset_at: None,
        }
    }
}

impl From<&GithubError> for ErrorBody {
    fn from(error: &GithubError) -> Self {
        let kind = match error {
            GithubError::Api(_) => "api",
            GithubError::Unauthorized => "unauthorized",
            GithubError::Forbidden => "forbidden",
            GithubError::RateLimited { .. } => "rate_limited",
            GithubError::Http(_) => "http",
            GithubError::Malformed => "malformed",
            GithubError::Transport => "transport",
            GithubError::NotFound => "not_found",
        };
        let mut body = Self::new(kind, error.to_string());
        if let GithubError::RateLimited {
            reset_at: Some(reset_at),
        } = error
        {
            body.reset_at = Some(date(*reset_at));
        }
        body
    }
}

impl From<&ConfigError> for ErrorBody {
    fn from(error: &ConfigError) -> Self {
        Self::new("config", error.to_string())
    }
}

impl From<&TokenError> for ErrorBody {
    fn from(error: &TokenError) -> Self {
        Self::new("token", error.to_string())
    }
}

impl From<&PortsError> for ErrorBody {
    fn from(error: &PortsError) -> Self {
        let kind = match error {
            PortsError::NotListening { .. } => "not_listening",
            PortsError::Denied { .. } => "permission",
            PortsError::System(_) => "system",
        };
        Self::new(kind, error.to_string())
    }
}

/// Écrit l'enveloppe et rend le texte avec la réussite, qui donne le code de
/// sortie.
pub fn envelope<T: Serialize>(result: Result<T, ErrorBody>) -> (String, bool) {
    let ok = result.is_ok();
    let (data, error) = match result {
        Ok(data) => (Some(data), None),
        Err(error) => (None, Some(error)),
    };
    let text = serde_json::to_string(&Envelope {
        version: FORMAT_VERSION,
        ok,
        data,
        error,
    })
    // Des chaînes, des nombres et des booléens : l'écriture ne peut pas
    // échouer.
    .expect("l'enveloppe s'écrit toujours");
    (text, ok)
}

#[derive(Serialize)]
pub struct ListOut {
    pull_requests: Vec<SummaryOut>,
    preferred_merge_method: &'static str,
    refresh_interval: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit: Option<RateLimitOut>,
}

#[derive(Serialize)]
struct RateLimitOut {
    remaining: u32,
    reset_at: String,
}

#[derive(Serialize)]
struct SummaryOut {
    repo: String,
    number: u32,
    title: String,
    author: String,
    url: String,
    is_draft: bool,
    checks: &'static str,
    review: &'static str,
    mergeable: &'static str,
    merge_state: &'static str,
    base_ref: String,
    head_ref: String,
    updated_at: String,
    merge_methods: Vec<&'static str>,
    check_runs: Vec<CheckOut>,
}

#[derive(Serialize)]
struct CheckOut {
    name: String,
    state: &'static str,
    url: Option<String>,
}

#[derive(Serialize)]
pub struct DetailOut {
    summary: SummaryOut,
    body: String,
    checks: Vec<CheckOut>,
    reviews: Vec<ReviewOut>,
    comments: Vec<CommentOut>,
    files: Vec<FileOut>,
    additions: u32,
    deletions: u32,
}

#[derive(Serialize)]
struct ReviewOut {
    author: String,
    state: &'static str,
    body: String,
    submitted_at: String,
}

#[derive(Serialize)]
struct CommentOut {
    author: String,
    body: String,
    created_at: String,
}

#[derive(Serialize)]
struct FileOut {
    path: String,
    additions: u32,
    deletions: u32,
}

/// Réponse d'une fusion réussie : rien à dire de plus que `ok`.
#[derive(Serialize)]
pub struct MergeOut {}

#[derive(Serialize)]
pub struct PortsOut {
    servers: Vec<ServerOut>,
    hidden: bool,
}

/// `project`, `directory` et `critical` sont écrits `null` quand ils
/// manquent : l'app lit toujours les mêmes clés.
#[derive(Serialize)]
struct ServerOut {
    port: u16,
    pid: u32,
    project: Option<String>,
    directory: Option<String>,
    command: String,
    started_at: String,
    user: String,
    critical: Option<CriticalOut>,
}

#[derive(Serialize)]
struct CriticalOut {
    reason: String,
}

#[derive(Serialize)]
pub struct StopOut {
    stopped: Vec<u32>,
    forced: bool,
}

pub fn ports_data(listing: &Listing) -> PortsOut {
    PortsOut {
        servers: listing
            .servers
            .iter()
            .map(|server| ServerOut {
                port: server.port,
                pid: server.pid,
                project: server.project.clone(),
                directory: server.directory.clone(),
                command: server.command.clone(),
                started_at: date(server.started_at),
                user: server.user.clone(),
                critical: server.critical.clone().map(|reason| CriticalOut { reason }),
            })
            .collect(),
        hidden: listing.hidden,
    }
}

pub fn stop_data(stopped: &Stopped) -> StopOut {
    StopOut {
        stopped: stopped.stopped.clone(),
        forced: stopped.forced,
    }
}

pub fn list_data(page: &ListPage, settings: &Config) -> ListOut {
    ListOut {
        pull_requests: page.pull_requests.iter().map(summary).collect(),
        preferred_merge_method: method(settings.preferred_merge_method),
        refresh_interval: settings.refresh_interval,
        rate_limit: page.rate_limit.map(|limit| RateLimitOut {
            remaining: limit.remaining,
            reset_at: date(limit.reset_at),
        }),
    }
}

pub fn detail_data(detail: &PrDetail) -> DetailOut {
    DetailOut {
        summary: summary(&detail.summary),
        body: detail.body.clone(),
        checks: detail.checks.iter().map(check).collect(),
        reviews: detail
            .reviews
            .iter()
            .map(|review| ReviewOut {
                author: review.author.clone(),
                state: review_state(review.state),
                body: review.body.clone(),
                submitted_at: date(review.submitted_at),
            })
            .collect(),
        comments: detail
            .comments
            .iter()
            .map(|comment| CommentOut {
                author: comment.author.clone(),
                body: comment.body.clone(),
                created_at: date(comment.created_at),
            })
            .collect(),
        files: detail
            .files
            .iter()
            .map(|file| FileOut {
                path: file.path.clone(),
                additions: file.additions,
                deletions: file.deletions,
            })
            .collect(),
        additions: detail.additions,
        deletions: detail.deletions,
    }
}

fn summary(pr: &PrSummary) -> SummaryOut {
    SummaryOut {
        repo: pr.key.repo.clone(),
        number: pr.key.number,
        title: pr.title.clone(),
        author: pr.author.clone(),
        url: pr.url.clone(),
        is_draft: pr.is_draft,
        checks: checks_state(pr.checks),
        review: review_state(pr.review),
        mergeable: mergeable_state(pr.mergeable),
        merge_state: merge_state(pr.merge_state),
        base_ref: pr.base_ref.clone(),
        head_ref: pr.head_ref.clone(),
        updated_at: date(pr.updated_at),
        merge_methods: pr.repo_rules.allowed().into_iter().map(method).collect(),
        check_runs: pr.check_runs.iter().map(check).collect(),
    }
}

fn check(run: &crate::model::CheckRun) -> CheckOut {
    CheckOut {
        name: run.name.clone(),
        state: checks_state(run.state),
        url: run.url.clone(),
    }
}

/// À la seconde, en UTC, suffixe `Z` : la forme que le décodeur ISO 8601 de
/// Swift lit sans réglage.
fn date(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn checks_state(state: ChecksState) -> &'static str {
    match state {
        ChecksState::Success => "success",
        ChecksState::Failure => "failure",
        ChecksState::Pending => "pending",
        ChecksState::None => "none",
    }
}

fn review_state(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Approved => "approved",
        ReviewState::ChangesRequested => "changes_requested",
        ReviewState::ReviewRequired => "review_required",
        ReviewState::None => "none",
    }
}

fn mergeable_state(state: MergeableState) -> &'static str {
    match state {
        MergeableState::Mergeable => "mergeable",
        MergeableState::Conflicting => "conflicting",
        MergeableState::Unknown => "unknown",
    }
}

fn merge_state(state: MergeState) -> &'static str {
    match state {
        MergeState::Clean => "clean",
        MergeState::Blocked => "blocked",
        MergeState::Unknown => "unknown",
    }
}

fn method(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "merge",
        MergeMethod::Squash => "squash",
        MergeMethod::Rebase => "rebase",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::github::dto::{DetailData, ListData};
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Deserialize)]
    struct ListEnvelope {
        data: ListData,
    }

    #[derive(Deserialize)]
    struct DetailEnvelope {
        data: DetailData,
    }

    const LIST: &str = include_str!("../../tests/fixtures/list.json");
    const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");

    fn page() -> ListPage {
        serde_json::from_str::<ListEnvelope>(LIST)
            .expect("la réponse enregistrée doit se lire")
            .data
            .to_list_page()
    }

    fn detail() -> PrDetail {
        serde_json::from_str::<DetailEnvelope>(DETAIL)
            .expect("la réponse enregistrée doit se lire")
            .data
            .repository
            .expect("dépôt présent")
            .pull_request
            .expect("pull request présente")
            .to_detail(page().pull_requests[0].clone())
    }

    fn ports_json() -> Value {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let listing = crate::ports::Listing {
            servers: crate::ports::servers(
                &crate::ports::recorded(),
                crate::ports::Scope::Node,
                501,
                now,
            ),
            hidden: false,
        };
        let (text, _) = envelope(Ok(ports_data(&listing)));
        serde_json::from_str(&text).unwrap()
    }

    fn stop_json() -> Value {
        let stopped = crate::ports::Stopped {
            stopped: vec![35815, 35834, 35860],
            forced: false,
        };
        let (text, _) = envelope(Ok(stop_data(&stopped)));
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn a_server_is_written_in_full() {
        let json = ports_json();
        assert_eq!(json["ok"], true);
        assert_eq!(json["data"]["hidden"], false);
        assert_eq!(
            json["data"]["servers"][0],
            serde_json::json!({
                "port": 3000,
                "pid": 35860,
                "project": "erp-menage",
                "directory": "/Users/antoinesouesme/dev/projets/gkfa/erp-menage",
                "command": "npm run dev",
                "started_at": "2026-10-14T20:31:47Z",
                "user": "antoinesouesme",
                "critical": null
            })
        );
    }

    #[test]
    fn a_critical_port_carries_its_reason_and_a_missing_project_is_null() {
        let now = chrono::Utc::now();
        let listing = crate::ports::Listing {
            servers: crate::ports::servers(
                &crate::ports::recorded(),
                crate::ports::Scope::Port(7000),
                501,
                now,
            ),
            hidden: false,
        };
        let (text, _) = envelope(Ok(ports_data(&listing)));
        let json: Value = serde_json::from_str(&text).unwrap();
        let server = &json["data"]["servers"][0];
        assert_eq!(
            server["critical"],
            serde_json::json!({ "reason": "AirPlay Receiver" })
        );
        assert_eq!(server["project"], Value::Null);
    }

    #[test]
    fn a_stop_lists_the_chain_and_whether_it_was_forced() {
        assert_eq!(
            stop_json()["data"],
            serde_json::json!({ "stopped": [35815, 35834, 35860], "forced": false })
        );
    }

    #[test]
    fn port_errors_have_their_own_kind() {
        use crate::ports::PortsError;
        for (error, kind) in [
            (
                PortsError::NotListening { pid: 1, port: 2 },
                "not_listening",
            ),
            (
                PortsError::Denied {
                    pid: 1,
                    user: "root".into(),
                },
                "permission",
            ),
            (PortsError::System("boom".into()), "system"),
        ] {
            let body = ErrorBody::from(&error);
            assert_eq!(body.kind, kind);
            assert_eq!(body.message, error.to_string());
        }
    }

    fn list_json() -> Value {
        let (text, ok) = envelope(Ok(list_data(&page(), &Config::default())));
        assert!(ok);
        serde_json::from_str(&text).expect("JSON valide")
    }

    fn detail_json() -> Value {
        let (text, ok) = envelope(Ok(detail_data(&detail())));
        assert!(ok);
        serde_json::from_str(&text).expect("JSON valide")
    }

    #[test]
    fn a_success_carries_the_version_and_the_data() {
        let json = list_json();
        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], true);
        assert!(json.get("error").is_none());
        assert_eq!(json["data"]["preferred_merge_method"], "squash");
        assert_eq!(json["data"]["refresh_interval"], 60);
        assert_eq!(json["data"]["rate_limit"]["remaining"], 4987);
    }

    #[test]
    fn the_first_pull_request_is_written_in_full() {
        let first = list_json()["data"]["pull_requests"][0].clone();
        assert_eq!(first["repo"], "moi/owl");
        assert_eq!(first["number"], 42);
        assert_eq!(first["title"], "Ajoute la fenêtre de fusion");
        assert_eq!(first["is_draft"], false);
        assert_eq!(first["checks"], "success");
        assert_eq!(first["review"], "approved");
        assert_eq!(first["mergeable"], "mergeable");
        assert_eq!(first["merge_state"], "clean");
        assert_eq!(first["base_ref"], "develop");
        assert_eq!(first["head_ref"], "feat/fusion");
        assert_eq!(first["updated_at"], "2026-08-30T09:12:44Z");
        assert_eq!(
            first["merge_methods"],
            serde_json::json!(["squash", "rebase"])
        );
        assert!(first["check_runs"].is_array());
    }

    #[test]
    fn the_detail_carries_its_summary_and_its_lists() {
        let data = detail_json()["data"].clone();
        assert_eq!(data["summary"]["number"], 42);
        assert_eq!(
            data["body"],
            "Ajoute la fenêtre de fusion et ses raccourcis."
        );
        assert_eq!(data["additions"], 214);
        assert_eq!(data["deletions"], 37);
        assert!(data["checks"]
            .as_array()
            .is_some_and(|list| !list.is_empty()));
        assert!(data["reviews"].is_array());
        assert!(data["comments"].is_array());
        assert!(data["files"].is_array());
        assert!(
            data.get("node_id").is_none(),
            "l'identifiant GraphQL reste chez owl"
        );
    }

    #[test]
    fn a_merge_success_is_an_empty_object() {
        let (text, ok) = envelope(Ok(MergeOut {}));
        assert!(ok);
        assert_eq!(text, r#"{"version":1,"ok":true,"data":{}}"#);
    }

    #[test]
    fn every_github_error_has_its_kind_and_its_message() {
        let cases = [
            (GithubError::Api("Base branch was modified.".into()), "api"),
            (GithubError::Unauthorized, "unauthorized"),
            (GithubError::Forbidden, "forbidden"),
            (GithubError::RateLimited { reset_at: None }, "rate_limited"),
            (GithubError::Http(502), "http"),
            (GithubError::Malformed, "malformed"),
            (GithubError::Transport, "transport"),
            (GithubError::NotFound, "not_found"),
        ];
        for (error, kind) in cases {
            let body = ErrorBody::from(&error);
            assert_eq!(body.kind, kind);
            assert_eq!(body.message, error.to_string());
        }
    }

    #[test]
    fn a_github_message_passes_verbatim() {
        let error = GithubError::Api("Base branch was modified.".into());
        let (text, ok) = envelope::<MergeOut>(Err(ErrorBody::from(&error)));
        assert!(!ok);
        let json: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], false);
        assert!(json.get("data").is_none());
        assert_eq!(json["error"]["kind"], "api");
        assert_eq!(json["error"]["message"], "Base branch was modified.");
        assert!(json["error"].get("reset_at").is_none());
    }

    #[test]
    fn a_rate_limit_carries_its_reset_time() {
        let reset_at = chrono::DateTime::parse_from_rfc3339("2026-10-13T14:32:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let body = ErrorBody::from(&GithubError::RateLimited {
            reset_at: Some(reset_at),
        });
        assert_eq!(body.reset_at.as_deref(), Some("2026-10-13T14:32:00Z"));
    }

    #[test]
    fn settings_and_token_errors_have_their_own_kind() {
        let config = ErrorBody::from(&ConfigError::EmptyFilters);
        assert_eq!(config.kind, "config");
        assert_eq!(config.message, ConfigError::EmptyFilters.to_string());
        let token = ErrorBody::from(&TokenError::GhMissing);
        assert_eq!(token.kind, "token");
        assert_eq!(token.message, TokenError::GhMissing.to_string());
    }

    /// Les exemples lus par les tests de l'app sont produits ici, à partir des
    /// mêmes réponses de GitHub. `OWL_UPDATE_FIXTURES=1 cargo test` les
    /// réécrit ; sans la variable, le test échoue s'ils ne sont plus à jour.
    #[test]
    fn the_app_fixtures_are_up_to_date() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("macos/Tests/OwlBarCoreTests/Fixtures");
        let expected = [
            ("list.json", pretty(&list_json())),
            ("detail.json", pretty(&detail_json())),
            ("ports.json", pretty(&ports_json())),
            ("stop.json", pretty(&stop_json())),
        ];
        let update = std::env::var("OWL_UPDATE_FIXTURES").is_ok();
        for (name, content) in expected {
            let path = directory.join(name);
            if update {
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(&path, &content).unwrap();
            }
            let current = std::fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(
                current, content,
                "{name} n'est plus à jour : relancer avec OWL_UPDATE_FIXTURES=1"
            );
        }
    }

    fn pretty(value: &Value) -> String {
        let mut text = serde_json::to_string_pretty(value).unwrap();
        text.push('\n');
        text
    }
}
