# Barre des menus — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Faire vivre `owl` dans la barre des menus de macOS : une app Swift affiche un compteur de pull requests et un panneau qui offre la liste, le détail et la fusion, en s'appuyant sur trois nouvelles sous-commandes JSON de `owl`.

**Architecture:** Côté Rust, un nouveau module `cli` lit les arguments, appelle `config`, `token` et `github` exactement comme la TUI, et écrit un document JSON versionné ; `main` lui passe la main avant toute prise du terminal. `github`, `app` et `ui` ne changent pas. Côté Swift, un paquet sous `macos/` sépare une bibliothèque `OwlBarCore` (lecture du JSON, lancement de `owl`, présentation, état — tout ce qui décide, testé) d'un exécutable `OwlBar` (vues SwiftUI, notifications, ouverture de session — sans test automatique).

**Tech Stack:** Rust édition 2021, `serde` (déjà présent, dérivation `Serialize`), `serde_json`, `chrono`. Swift 6.4 en mode de langage 5, SwiftUI `MenuBarExtra`, `UserNotifications`, `ServiceManagement`, XCTest. Aucune nouvelle dépendance Rust, aucune dépendance Swift externe.

**Spec:** `docs/superpowers/specs/2026-10-13-barre-des-menus-design.md`

## Conditions d'exécution

- Branche de travail : `feature/icone-barre-menus`, déjà créée depuis `develop` et portant la spec. Jamais de travail direct sur `develop`, jamais de pull request vers `main`.
- La spec et ce plan sont déjà commités sur la branche.
- Registre `sdd` : créer un registre neuf pour `2026-10-13-barre-des-menus` sous `.superpowers/sdd/`, ne lire ni modifier les registres existants.
- `docs/suivi/TODO.md` est hors périmètre. Une décision mise de côté et critique pour la suite se consigne dans `docs/suivi/DETTE.md`, au format déjà en place.
- Aucune question à poser avant de commencer. Une question n'est légitime que devant un blocage réel (outil absent, `swift test` impossible à lancer).
- À la fin : pull request vers `develop`, puis un rapport court qui dit ce qui reste à vérifier à la main.

## Global Constraints

- Le code est en anglais (identifiants, tests, interface, messages). Les commentaires, documents et messages de commit sont en français.
- `owl` sans argument se comporte exactement comme avant : la TUI.
- Dépendances Rust à sens unique : `cli` dépend de `model`, `filter`, `config`, `token` et `github`, jamais de `app` ni `ui`. `github` ne change pas.
- Enveloppe JSON : `{ "version": 1, "ok": true, "data": … }` ou `{ "version": 1, "ok": false, "error": { "kind", "message", "reset_at"? } }`. Code de sortie 0 si `ok`, 1 sinon. Erreur d'usage : message sur la sortie d'erreur, code 2, rien sur la sortie standard.
- Valeurs d'état en minuscules : `success`, `failure`, `pending`, `none` ; `approved`, `changes_requested`, `review_required`, `none` ; `mergeable`, `conflicting`, `unknown` ; `clean`, `blocked`, `unknown` ; méthodes `merge`, `squash`, `rebase`. Dates ISO 8601 en UTC, à la seconde, suffixe `Z`.
- `kind` vaut l'un de : `api`, `unauthorized`, `forbidden`, `rate_limited`, `http`, `malformed`, `transport`, `not_found`, `config`, `token`.
- Les messages d'erreur de `owl` et de GitHub passent tels quels jusqu'au panneau.
- Le jeton n'apparaît jamais dans le JSON, sur la sortie d'erreur ni dans un message.
- Messages de l'app, au caractère près : « owl not found. Install it with `cargo install --path .` », « This owl version is not supported by the menu bar app. Update both. », « Unreadable answer from owl. », « No merge method allowed on this repository. », titre de notification « Ready to merge », corps « `<repo> #<number> · <title>` ».
- Compteur de la barre : `🦉 <prêtes> · <autres>` ; `🦉 ⚠` après une lecture échouée ; `🦉 …` avant la première lecture.
- Ordre des méthodes de fusion dans l'app : merge, squash, rebase (celui de `RepoMergeRules::allowed`).
- Dans `OwlBar` (les vues), aucune décision : pas de pictogramme, de libellé ni de format choisi dans une vue. Tout vient de `OwlBarCore`.
- Commandes qui doivent passer à la fin de chaque tâche qui touche le Rust : `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`. À la fin de chaque tâche qui touche le Swift : `swift build` et `swift test` dans `macos/`.

## Review Focus

- `refresh_interval = 0` dans la configuration : l'app ne rafraîchit plus d'elle-même, mais le bouton de rafraîchissement marche toujours. Test dans la tâche 6.
- Limite d'appels atteinte avec `reset_at` : aucune relance (automatique ou manuelle) avant l'heure donnée, puis reprise normale. Test dans la tâche 6.
- Clic sur rafraîchir pendant qu'une lecture automatique tourne : un seul `owl list` lancé. Test dans la tâche 6.
- Retour à la liste pendant que le détail charge, puis ouverture d'une autre PR : la réponse tardive de la première ne s'affiche jamais. Test dans la tâche 6.
- `owl` qui répond autre chose que du JSON (erreur d'usage d'un `owl` trop ancien qui ne connaît pas `list`, sortie vide) : message « Unreadable answer from owl. » et icône en alerte, sans plantage. Tests dans les tâches 4 et 6.

---

## Structure des fichiers

Rust :

- Create `src/cli/mod.rs` — lecture des arguments (`parse`), exécution (`run`), recherche d'une PR dans la liste (`find`), rendu de l'enveloppe (`render`).
- Create `src/cli/output.rs` — structures écrites en JSON et conversions depuis `model` et les erreurs.
- Modify `src/main.rs` — aiguillage vers `cli` avant tout le reste.

Swift (`macos/`) :

- `Package.swift`
- `Sources/OwlBarCore/Models.swift` — types lus dans le JSON.
- `Sources/OwlBarCore/Decoding.swift` — enveloppe, version, erreurs.
- `Sources/OwlBarCore/Runner.swift` — recherche et lancement de `owl`.
- `Sources/OwlBarCore/Presentation.swift` — pictogrammes, libellés, âge, compteur, lignes, détail, choix de fusion.
- `Sources/OwlBarCore/ReadyTracker.swift` — détection des PR devenues prêtes.
- `Sources/OwlBarCore/BarState.swift` — état observable du panneau.
- `Sources/OwlBar/OwlBarApp.swift`, `PanelView.swift`, `ListView.swift`, `DetailView.swift`, `SystemNotifier.swift`, `LaunchAtLogin.swift`.
- `Tests/OwlBarCoreTests/*.swift` et `Tests/OwlBarCoreTests/Fixtures/{list,detail}.json` (produits par un test Rust).
- `Resources/Info.plist`, `build.sh`.

---

### Task 1: Lecture des arguments et aiguillage dans `main`

**Files:**
- Create: `src/cli/mod.rs`
- Modify: `src/main.rs:1-12` (déclaration du module) et `src/main.rs:30-31` (début de `main`)

**Interfaces:**
- Produces:
  - `pub enum Request { List, Detail(PrKey), Merge(PrKey, MergeMethod) }` (dérive `Debug, PartialEq`)
  - `pub fn parse(args: &[String]) -> Result<Option<Request>, UsageError>` — `Ok(None)` sans argument (TUI).
  - `pub struct UsageError;` qui implémente `Display` avec le texte `USAGE`.
  - `pub fn run(request: Request) -> ExitCode` (corps provisoire dans cette tâche, complété en tâche 3).

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `src/cli/mod.rs` avec seulement les tests :

```rust
//! Sous-commandes non interactives de `owl`, lues par l'app de la barre des
//! menus. Chacune écrit un seul document JSON et se termine ; l'écran n'est
//! jamais pris.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MergeMethod, PrKey};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    fn key() -> PrKey {
        PrKey {
            repo: "moi/owl".to_string(),
            number: 42,
        }
    }

    #[test]
    fn no_argument_means_the_tui() {
        assert_eq!(parse(&args(&[])), Ok(None));
    }

    #[test]
    fn list_takes_no_argument() {
        assert_eq!(parse(&args(&["list"])), Ok(Some(Request::List)));
        assert_eq!(parse(&args(&["list", "extra"])), Err(UsageError));
    }

    #[test]
    fn detail_takes_a_repo_and_a_number() {
        assert_eq!(
            parse(&args(&["detail", "moi/owl", "42"])),
            Ok(Some(Request::Detail(key())))
        );
    }

    #[test]
    fn merge_takes_a_repo_a_number_and_a_method() {
        for (word, method) in [
            ("merge", MergeMethod::Merge),
            ("squash", MergeMethod::Squash),
            ("rebase", MergeMethod::Rebase),
        ] {
            assert_eq!(
                parse(&args(&["merge", "moi/owl", "42", word])),
                Ok(Some(Request::Merge(key(), method)))
            );
        }
    }

    #[test]
    fn malformed_arguments_are_a_usage_error() {
        for bad in [
            &["detail"][..],
            &["detail", "moi/owl"],
            &["detail", "owl", "42"],
            &["detail", "/owl", "42"],
            &["detail", "moi/", "42"],
            &["detail", "moi/owl/x", "42"],
            &["detail", "moi/owl", "abc"],
            &["detail", "moi/owl", "0"],
            &["detail", "moi/owl", "-3"],
            &["merge", "moi/owl", "42"],
            &["merge", "moi/owl", "42", "fast-forward"],
            &["unknown"],
        ] {
            assert_eq!(parse(&args(bad)), Err(UsageError), "{bad:?}");
        }
    }

    #[test]
    fn the_usage_message_names_every_subcommand() {
        let text = UsageError.to_string();
        assert!(text.contains("owl list"));
        assert!(text.contains("owl detail <owner/repo> <number>"));
        assert!(text.contains("owl merge <owner/repo> <number> <merge|squash|rebase>"));
    }
}
```

Dans `src/main.rs`, ajouter `mod cli;` en tête de la liste des modules (ordre alphabétique : avant `mod config;`).

- [ ] **Step 2: Vérifier l'échec**

Run: `cargo test cli::`
Expected: échec de compilation, `parse`, `Request` et `UsageError` introuvables.

- [ ] **Step 3: Écrire l'implémentation**

Au-dessus du module de tests de `src/cli/mod.rs` :

```rust
use std::fmt;
use std::process::ExitCode;

use crate::model::{MergeMethod, PrKey};

/// Ce que demande la ligne de commande, quand elle demande autre chose que
/// la TUI.
#[derive(Debug, PartialEq)]
pub enum Request {
    List,
    Detail(PrKey),
    Merge(PrKey, MergeMethod),
}

const USAGE: &str = "Usage: owl [list | detail <owner/repo> <number> | merge <owner/repo> <number> <merge|squash|rebase>]
  owl list
  owl detail <owner/repo> <number>
  owl merge <owner/repo> <number> <merge|squash|rebase>";

/// Arguments incompréhensibles. Le texte d'aide est le seul message : il dit
/// à lui seul ce qui était attendu.
#[derive(Debug, PartialEq)]
pub struct UsageError;

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(USAGE)
    }
}

/// Lit les arguments, nom du programme exclu. `Ok(None)` : aucun argument,
/// c'est la TUI qui démarre.
pub fn parse(args: &[String]) -> Result<Option<Request>, UsageError> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Ok(None),
        ["list"] => Ok(Some(Request::List)),
        ["detail", repo, number] => Ok(Some(Request::Detail(parse_key(repo, number)?))),
        ["merge", repo, number, method] => Ok(Some(Request::Merge(
            parse_key(repo, number)?,
            parse_method(method)?,
        ))),
        _ => Err(UsageError),
    }
}

/// `org/dépôt` avec exactement une barre et deux moitiés non vides, et un
/// numéro strictement positif.
fn parse_key(repo: &str, number: &str) -> Result<PrKey, UsageError> {
    let mut parts = repo.split('/');
    let valid = matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty()
    );
    let number: u32 = number.parse().map_err(|_| UsageError)?;
    if !valid || number == 0 {
        return Err(UsageError);
    }
    Ok(PrKey {
        repo: repo.to_string(),
        number,
    })
}

fn parse_method(word: &str) -> Result<MergeMethod, UsageError> {
    match word {
        "merge" => Ok(MergeMethod::Merge),
        "squash" => Ok(MergeMethod::Squash),
        "rebase" => Ok(MergeMethod::Rebase),
        _ => Err(UsageError),
    }
}

/// Exécute la demande et rend le code de sortie. Complété à la tâche 3.
pub fn run(request: Request) -> ExitCode {
    let _ = request;
    ExitCode::FAILURE
}
```

Dans `src/main.rs`, au tout début de `fn main()`, avant le chargement des réglages :

```rust
    // Une sous-commande passe la main à `cli` avant toute autre chose : le
    // terminal n'est jamais pris, et la sortie reste un document JSON.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse(&args) {
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
        Ok(Some(request)) => return cli::run(request),
        Ok(None) => {}
    }
```

- [ ] **Step 4: Vérifier le passage**

Run: `cargo test cli::`
Expected: 6 tests PASS.

Run: `cargo run -- nonsense; echo "exit=$?"`
Expected: le texte d'usage sur la sortie d'erreur, `exit=2`.

- [ ] **Step 5: Commandes du projet**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe. Si `clippy` signale `run` comme inutilisée ou le paramètre ignoré, ce n'est pas le cas : `main` l'appelle.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs src/main.rs
git commit -m "Lecture des sous-commandes de owl

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Le format JSON et les exemples pour l'app

**Files:**
- Create: `src/cli/output.rs`
- Modify: `src/cli/mod.rs` (ajout de `mod output;`)
- Create (par le test) : `macos/Tests/OwlBarCoreTests/Fixtures/list.json`, `macos/Tests/OwlBarCoreTests/Fixtures/detail.json`

**Interfaces:**
- Consumes: `model::{PrSummary, PrDetail, ListPage, ...}`, `github::GithubError`, `config::ConfigError`, `token::TokenError`.
- Produces (dans `cli::output`) :
  - `pub const FORMAT_VERSION: u32 = 1;`
  - `pub struct ErrorBody { pub kind: &'static str, pub message: String, pub reset_at: Option<String> }` avec `impl From<&GithubError>`, `From<&ConfigError>`, `From<&TokenError>`.
  - `pub fn list_data(page: &ListPage, settings: &Config) -> ListOut`
  - `pub fn detail_data(detail: &PrDetail) -> DetailOut`
  - `pub struct MergeOut {}`
  - `pub fn envelope<T: Serialize>(result: Result<T, ErrorBody>) -> (String, bool)` — le texte JSON et `ok`.

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `src/cli/output.rs` avec les tests :

```rust
//! Forme du JSON écrit par les sous-commandes. Ce fichier est le contrat lu
//! par l'app de la barre des menus : tout changement de forme passe par une
//! hausse de `FORMAT_VERSION`.

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
        assert_eq!(first["merge_methods"], serde_json::json!(["squash", "rebase"]));
        assert!(first["check_runs"].is_array());
    }

    #[test]
    fn the_detail_carries_its_summary_and_its_lists() {
        let data = detail_json()["data"].clone();
        assert_eq!(data["summary"]["number"], 42);
        assert_eq!(data["body"], "Ajoute la fenêtre de fusion et ses raccourcis.");
        assert_eq!(data["additions"], 214);
        assert_eq!(data["deletions"], 37);
        assert!(data["checks"].as_array().is_some_and(|list| !list.is_empty()));
        assert!(data["reviews"].is_array());
        assert!(data["comments"].is_array());
        assert!(data["files"].is_array());
        assert!(data.get("node_id").is_none(), "l'identifiant GraphQL reste chez owl");
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
```

Dans `src/cli/mod.rs`, ajouter `mod output;` sous le commentaire de tête.

Vérifier que `crate::github::dto` est accessible depuis `cli` : `src/github/mod.rs` le déclare `pub mod dto;`, c'est le cas.

- [ ] **Step 2: Vérifier l'échec**

Run: `cargo test cli::output`
Expected: échec de compilation, `envelope`, `list_data`, `ErrorBody`… introuvables.

- [ ] **Step 3: Écrire l'implémentation**

Au-dessus des tests de `src/cli/output.rs` :

```rust
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::config::{Config, ConfigError};
use crate::github::GithubError;
use crate::model::{
    ChecksState, ListPage, MergeMethod, MergeState, MergeableState, PrDetail, PrSummary,
    ReviewState,
};
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
```

Si le compilateur signale `ListOut`, `DetailOut` ou une fonction comme inutilisés hors des tests, ajouter `#[allow(dead_code)]` **n'est pas** la solution : la tâche 3 les utilise. Faire la tâche 3 dans la foulée avant `clippy`, ou, si la revue de tâche l'exige, lancer `clippy` avec la tâche 3 seulement.

- [ ] **Step 4: Produire les exemples pour l'app**

Run: `OWL_UPDATE_FIXTURES=1 cargo test cli::output::tests::the_app_fixtures_are_up_to_date`
Expected: PASS, et les deux fichiers `macos/Tests/OwlBarCoreTests/Fixtures/list.json` et `detail.json` existent.

- [ ] **Step 5: Vérifier le passage**

Run: `cargo test cli::`
Expected: tous les tests PASS, celui des exemples compris (sans la variable).

Ouvrir `macos/Tests/OwlBarCoreTests/Fixtures/list.json` et vérifier à l'œil : 5 pull requests, aucune trace de jeton, des dates en `…Z`.

- [ ] **Step 6: Commit**

```bash
git add src/cli/ macos/Tests/OwlBarCoreTests/Fixtures/
git commit -m "Format JSON des sous-commandes et exemples pour l'app

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Exécution des sous-commandes

**Files:**
- Modify: `src/cli/mod.rs` (remplacer le `run` provisoire, ajouter `find` et `perform`)

**Interfaces:**
- Consumes: `output::{envelope, list_data, detail_data, MergeOut, ErrorBody}` (tâche 2), `config::load`, `token::resolve`, `github::Client::{new, fetch_pull_requests, fetch_detail, merge_pull_request}`, `filter::{Filter, build_query}`.
- Produces: `pub fn run(request: Request) -> ExitCode` qui écrit une enveloppe sur la sortie standard ; `fn find(page: ListPage, key: &PrKey) -> Result<PrSummary, GithubError>`.

- [ ] **Step 1: Écrire les tests qui échouent**

Ajouter au module de tests de `src/cli/mod.rs` :

```rust
    use crate::github::GithubError;
    use crate::model::ListPage;

    fn recorded_page() -> ListPage {
        #[derive(serde::Deserialize)]
        struct Envelope {
            data: crate::github::dto::ListData,
        }
        serde_json::from_str::<Envelope>(include_str!("../../tests/fixtures/list.json"))
            .expect("la réponse enregistrée doit se lire")
            .data
            .to_list_page()
    }

    #[test]
    fn find_returns_the_summary_with_that_key() {
        let summary = find(recorded_page(), &key()).expect("présente");
        assert_eq!(summary.key, key());
        assert_eq!(summary.title, "Ajoute la fenêtre de fusion");
    }

    #[test]
    fn a_key_outside_the_list_is_not_found() {
        let missing = PrKey {
            repo: "moi/owl".to_string(),
            number: 9999,
        };
        assert!(matches!(
            find(recorded_page(), &missing),
            Err(GithubError::NotFound)
        ));
    }

    #[test]
    fn the_query_is_built_from_the_settings() {
        let settings = crate::config::Config::default();
        assert_eq!(
            query(&settings),
            crate::filter::build_query(
                &settings
                    .filters
                    .iter()
                    .map(|text| crate::filter::Filter::parse(text))
                    .collect::<Vec<_>>()
            )
        );
    }
```

- [ ] **Step 2: Vérifier l'échec**

Run: `cargo test cli::tests`
Expected: échec de compilation, `find` et `query` introuvables.

- [ ] **Step 3: Écrire l'implémentation**

Dans `src/cli/mod.rs`, compléter les imports :

```rust
use std::fmt;
use std::process::ExitCode;

use serde::Serialize;

use crate::config::{self, Config};
use crate::filter::{self, Filter};
use crate::github::{Client, GithubError};
use crate::model::{ListPage, MergeMethod, PrKey, PrSummary};
use crate::token;

use output::{ErrorBody, MergeOut};
```

Remplacer le `run` provisoire par :

```rust
/// Exécute la demande, écrit l'enveloppe sur la sortie standard et rend le
/// code de sortie : 0 si `ok`, 1 sinon. Toute erreur passe par l'enveloppe,
/// réglages et jeton compris : l'app n'a qu'un format à lire.
pub fn run(request: Request) -> ExitCode {
    let (text, ok) = match request {
        Request::List => output::envelope(perform(|settings, client| async move {
            let page = client
                .fetch_pull_requests(&query(&settings), settings.page_size)
                .await
                .map_err(|error| ErrorBody::from(&error))?;
            Ok(output::list_data(&page, &settings))
        })),
        Request::Detail(key) => output::envelope(perform(|settings, client| async move {
            let summary = summary_of(&settings, &client, &key).await?;
            let detail = client
                .fetch_detail(&summary)
                .await
                .map_err(|error| ErrorBody::from(&error))?;
            Ok(output::detail_data(&detail))
        })),
        Request::Merge(key, method) => output::envelope(perform(|settings, client| async move {
            let summary = summary_of(&settings, &client, &key).await?;
            client
                .merge_pull_request(&summary, None, method)
                .await
                .map_err(|error| ErrorBody::from(&error))?;
            Ok(MergeOut {})
        })),
    };
    println!("{text}");
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Prépare réglages, jeton et client, puis lance le travail dans un
/// exécuteur. Le jeton entre dans le client et n'en ressort pas.
fn perform<T, F, Fut>(work: F) -> Result<T, ErrorBody>
where
    T: Serialize,
    F: FnOnce(Config, Client) -> Fut,
    Fut: std::future::Future<Output = Result<T, ErrorBody>>,
{
    let settings = config::load().map_err(|error| ErrorBody::from(&error))?;
    let token = token::resolve().map_err(|error| ErrorBody::from(&error))?;
    let client = Client::new(token.expose()).map_err(|error| ErrorBody::from(&error))?;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| ErrorBody::from(&GithubError::Transport))?;
    runtime.block_on(work(settings, client))
}

/// Chaîne de recherche, assemblée comme la TUI l'assemble.
fn query(settings: &Config) -> String {
    let filters: Vec<Filter> = settings
        .filters
        .iter()
        .map(|text| Filter::parse(text))
        .collect();
    filter::build_query(&filters)
}

/// Relance la liste et y cherche la clé : `github` attend un résumé, et
/// l'app ne montre que des pull requests de cette liste.
async fn summary_of(
    settings: &Config,
    client: &Client,
    key: &PrKey,
) -> Result<PrSummary, ErrorBody> {
    let page = client
        .fetch_pull_requests(&query(settings), settings.page_size)
        .await
        .map_err(|error| ErrorBody::from(&error))?;
    find(page, key).map_err(|error| ErrorBody::from(&error))
}

/// La pull request de cette clé dans la liste, ou `NotFound` si elle en est
/// sortie.
fn find(page: ListPage, key: &PrKey) -> Result<PrSummary, GithubError> {
    page.pull_requests
        .into_iter()
        .find(|pr| &pr.key == key)
        .ok_or(GithubError::NotFound)
}
```

Si `Filter::parse` prend `&str` et que `.map(|text| Filter::parse(text))` déclenche `clippy::redundant_closure`, écrire `.map(|text| Filter::parse(text.as_str()))` comme le fait `src/app/mod.rs:349`, ou reprendre exactement la forme de cette ligne.

- [ ] **Step 4: Vérifier le passage**

Run: `cargo test cli::`
Expected: PASS.

- [ ] **Step 5: Essai réel**

Run: `cargo run -- list | head -c 400; echo; cargo run -- list >/dev/null; echo "exit=$?"`
Expected: un document qui commence par `{"version":1,"ok":true,"data":{"pull_requests":[`, `exit=0`.

Run: `cargo run -- detail moi/inexistant 1; echo "exit=$?"`
Expected: `{"version":1,"ok":false,"error":{"kind":"not_found","message":"Pull request not found."}}`, `exit=1`.

Run: `env -u GITHUB_TOKEN OWL_TOKEN=x cargo run -- list; echo "exit=$?"`
Expected: `kind` à `unauthorized`, `exit=1`, et la chaîne `x` n'apparaît nulle part comme jeton dans la sortie.

Ne pas lancer `owl merge` sur une vraie pull request pendant ce plan.

- [ ] **Step 6: Commandes du projet**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe.

- [ ] **Step 7: Commit**

```bash
git add src/cli/mod.rs
git commit -m "Exécution des sous-commandes list, detail et merge

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Paquet Swift, modèles et lecture du JSON

**Files:**
- Create: `macos/Package.swift`
- Create: `macos/Sources/OwlBarCore/Models.swift`
- Create: `macos/Sources/OwlBarCore/Decoding.swift`
- Create: `macos/Sources/OwlBar/OwlBarApp.swift` (provisoire, remplacé en tâche 7)
- Create: `macos/Tests/OwlBarCoreTests/DecodingTests.swift`
- Modify: `.gitignore`

**Interfaces:**
- Consumes: les exemples `Fixtures/list.json` et `Fixtures/detail.json` (tâche 2).
- Produces:
  - Types publics : `PrKey`, `ChecksState`, `ReviewState`, `MergeableState`, `MergeVerdict`, `MergeMethod`, `CheckRun`, `PullRequest`, `RateLimit`, `ListData`, `Review`, `Comment`, `ChangedFile`, `PrDetail`, `MergeData`.
  - `public struct OwlFailure: Error, Equatable, Decodable { kind: String; message: String; resetAt: Date? }` avec `OwlFailure.notFound`, `.unsupported`, `.unreadable`.
  - `public func decodeOwl<T: Decodable>(_ type: T.Type, from data: Data) -> Result<T, OwlFailure>`

- [ ] **Step 1: Créer le paquet**

`macos/Package.swift` :

```swift
// swift-tools-version:5.10
import PackageDescription

let package = Package(
    name: "OwlBar",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "OwlBarCore"),
        .executableTarget(name: "OwlBar", dependencies: ["OwlBarCore"]),
        .testTarget(
            name: "OwlBarCoreTests",
            dependencies: ["OwlBarCore"],
            resources: [.copy("Fixtures")]
        ),
    ]
)
```

`macos/Sources/OwlBar/OwlBarApp.swift` provisoire, pour que le paquet compile :

```swift
// Point d'entrée provisoire, remplacé par l'app de la barre des menus.
import OwlBarCore

print("OwlBar")
```

Ajouter à `.gitignore`, sous la section Rust :

```
# App de la barre des menus
/macos/.build/
/macos/build/
```

- [ ] **Step 2: Écrire les tests qui échouent**

`macos/Tests/OwlBarCoreTests/DecodingTests.swift` :

```swift
import XCTest
@testable import OwlBarCore

func fixture(_ name: String) throws -> Data {
    let url = try XCTUnwrap(
        Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "Fixtures")
    )
    return try Data(contentsOf: url)
}

final class DecodingTests: XCTestCase {
    func testTheRecordedListIsReadInFull() throws {
        let list = try decodeOwl(ListData.self, from: fixture("list")).get()
        XCTAssertEqual(list.pullRequests.count, 5)
        XCTAssertEqual(list.preferredMergeMethod, .squash)
        XCTAssertEqual(list.refreshInterval, 60)
        let first = list.pullRequests[0]
        XCTAssertEqual(first.key, PrKey(repo: "moi/owl", number: 42))
        XCTAssertEqual(first.checks, .success)
        XCTAssertEqual(first.review, .approved)
        XCTAssertEqual(first.mergeState, .clean)
        XCTAssertEqual(first.mergeMethods, [.squash, .rebase])
        XCTAssertEqual(first.baseRef, "develop")
    }

    func testTheRecordedDetailIsReadInFull() throws {
        let detail = try decodeOwl(PrDetail.self, from: fixture("detail")).get()
        XCTAssertEqual(detail.summary.number, 42)
        XCTAssertEqual(detail.additions, 214)
        XCTAssertFalse(detail.checks.isEmpty)
    }

    func testAMergeSuccessIsAnEmptyObject() throws {
        let data = Data(#"{"version":1,"ok":true,"data":{}}"#.utf8)
        XCTAssertNoThrow(try decodeOwl(MergeData.self, from: data).get())
    }

    func testAnErrorEnvelopeGivesOwlsMessageVerbatim() {
        let data = Data(#"{"version":1,"ok":false,"error":{"kind":"api","message":"Base branch was modified."}}"#.utf8)
        let result = decodeOwl(MergeData.self, from: data)
        XCTAssertEqual(result, .failure(OwlFailure(kind: "api", message: "Base branch was modified.", resetAt: nil)))
    }

    func testARateLimitCarriesItsResetTime() throws {
        let data = Data(#"{"version":1,"ok":false,"error":{"kind":"rate_limited","message":"Rate limit reached.","reset_at":"2026-10-13T14:32:00Z"}}"#.utf8)
        guard case .failure(let failure) = decodeOwl(ListData.self, from: data) else {
            return XCTFail("échec attendu")
        }
        XCTAssertEqual(failure.resetAt, ISO8601DateFormatter().date(from: "2026-10-13T14:32:00Z"))
    }

    func testAnotherFormatVersionIsRefused() {
        let data = Data(#"{"version":2,"ok":true,"data":{}}"#.utf8)
        XCTAssertEqual(decodeOwl(MergeData.self, from: data), .failure(.unsupported))
    }

    func testAnythingButJsonIsUnreadable() {
        for text in ["", "Usage: owl [list | …]", "{\"ok\":true}"] {
            XCTAssertEqual(decodeOwl(MergeData.self, from: Data(text.utf8)), .failure(.unreadable), text)
        }
    }

    func testTheFixedMessagesAreExact() {
        XCTAssertEqual(OwlFailure.notFound.message, "owl not found. Install it with `cargo install --path .`")
        XCTAssertEqual(OwlFailure.unsupported.message, "This owl version is not supported by the menu bar app. Update both.")
        XCTAssertEqual(OwlFailure.unreadable.message, "Unreadable answer from owl.")
    }
}
```

`Result` n'est `Equatable` que si ses deux paramètres le sont : `ListData`, `PrDetail` et `MergeData` doivent être `Equatable`.

- [ ] **Step 3: Vérifier l'échec**

Run: `cd macos && swift test`
Expected: échec de compilation, `decodeOwl`, `ListData`… introuvables.

- [ ] **Step 4: Écrire les modèles**

`macos/Sources/OwlBarCore/Models.swift` :

```swift
// Types lus dans le JSON de `owl`. Ils suivent le contrat de
// `src/cli/output.rs` ; les noms en snake_case sont convertis à la lecture.
import Foundation

public struct PrKey: Hashable, Sendable {
    public let repo: String
    public let number: Int

    public init(repo: String, number: Int) {
        self.repo = repo
        self.number = number
    }
}

public enum ChecksState: String, Decodable, Sendable {
    case success, failure, pending, none
}

public enum ReviewState: String, Decodable, Sendable {
    case approved
    case changesRequested = "changes_requested"
    case reviewRequired = "review_required"
    case none
}

public enum MergeableState: String, Decodable, Sendable {
    case mergeable, conflicting, unknown
}

/// Verdict de fusion de GitHub. `clean` : plus rien ne bloque.
public enum MergeVerdict: String, Decodable, Sendable {
    case clean, blocked, unknown
}

/// Dans l'ordre d'affichage : merge, squash, rebase.
public enum MergeMethod: String, Decodable, Sendable, CaseIterable {
    case merge, squash, rebase
}

public struct CheckRun: Decodable, Equatable, Sendable {
    public let name: String
    public let state: ChecksState
    public let url: String?
}

public struct PullRequest: Decodable, Equatable, Sendable, Identifiable {
    public let repo: String
    public let number: Int
    public let title: String
    public let author: String
    public let url: String
    public let isDraft: Bool
    public let checks: ChecksState
    public let review: ReviewState
    public let mergeable: MergeableState
    public let mergeState: MergeVerdict
    public let baseRef: String
    public let headRef: String
    public let updatedAt: Date
    public let mergeMethods: [MergeMethod]
    public let checkRuns: [CheckRun]

    public var key: PrKey { PrKey(repo: repo, number: number) }
    public var id: PrKey { key }
}

public struct RateLimit: Decodable, Equatable, Sendable {
    public let remaining: Int
    public let resetAt: Date
}

public struct ListData: Decodable, Equatable, Sendable {
    public let pullRequests: [PullRequest]
    public let preferredMergeMethod: MergeMethod
    public let refreshInterval: Int
    public let rateLimit: RateLimit?
}

public struct Review: Decodable, Equatable, Sendable {
    public let author: String
    public let state: ReviewState
    public let body: String
    public let submittedAt: Date
}

public struct Comment: Decodable, Equatable, Sendable {
    public let author: String
    public let body: String
    public let createdAt: Date
}

public struct ChangedFile: Decodable, Equatable, Sendable {
    public let path: String
    public let additions: Int
    public let deletions: Int
}

public struct PrDetail: Decodable, Equatable, Sendable {
    public let summary: PullRequest
    public let body: String
    public let checks: [CheckRun]
    public let reviews: [Review]
    public let comments: [Comment]
    public let files: [ChangedFile]
    public let additions: Int
    public let deletions: Int
}

/// Réponse d'une fusion réussie : un objet vide.
public struct MergeData: Decodable, Equatable, Sendable {}
```

- [ ] **Step 5: Écrire la lecture de l'enveloppe**

`macos/Sources/OwlBarCore/Decoding.swift` :

```swift
// Lecture de l'enveloppe écrite par `owl`. Toute réponse qui n'en est pas
// une devient une erreur affichable : l'app ne plante jamais sur une sortie
// inattendue.
import Foundation

/// Seule version du format que l'app sait lire.
public let supportedFormatVersion = 1

/// Erreur affichable. `message` est montré tel quel ; `kind` sert à réagir
/// sans lire le texte.
public struct OwlFailure: Error, Equatable, Decodable, Sendable {
    public let kind: String
    public let message: String
    public let resetAt: Date?

    public init(kind: String, message: String, resetAt: Date?) {
        self.kind = kind
        self.message = message
        self.resetAt = resetAt
    }

    public static let notFound = OwlFailure(
        kind: "owl_not_found",
        message: "owl not found. Install it with `cargo install --path .`",
        resetAt: nil
    )
    public static let unsupported = OwlFailure(
        kind: "unsupported_version",
        message: "This owl version is not supported by the menu bar app. Update both.",
        resetAt: nil
    )
    public static let unreadable = OwlFailure(
        kind: "unreadable",
        message: "Unreadable answer from owl.",
        resetAt: nil
    )
}

private struct Head: Decodable {
    let version: Int
}

private struct Body<T: Decodable>: Decodable {
    let ok: Bool
    let data: T?
    let error: OwlFailure?
}

public func decodeOwl<T: Decodable>(_ type: T.Type, from data: Data) -> Result<T, OwlFailure> {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    decoder.dateDecodingStrategy = .iso8601

    // La version est lue seule d'abord : un format inconnu doit être refusé
    // même si le reste ne se lit pas.
    guard let head = try? decoder.decode(Head.self, from: data) else {
        return .failure(.unreadable)
    }
    guard head.version == supportedFormatVersion else {
        return .failure(.unsupported)
    }
    guard let body = try? decoder.decode(Body<T>.self, from: data) else {
        return .failure(.unreadable)
    }
    if body.ok, let value = body.data {
        return .success(value)
    }
    return .failure(body.error ?? .unreadable)
}
```

- [ ] **Step 6: Vérifier le passage**

Run: `cd macos && swift build && swift test`
Expected: les 8 tests de `DecodingTests` PASS. En cas d'échec de lecture du détail sur un champ précis, comparer avec `Fixtures/detail.json` : c'est le modèle Swift qu'on corrige, pas l'exemple.

- [ ] **Step 7: Commit**

```bash
git add .gitignore macos/Package.swift macos/Sources macos/Tests/OwlBarCoreTests/DecodingTests.swift
git commit -m "Paquet Swift de l'app et lecture du JSON de owl

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Trouver et lancer `owl`

**Files:**
- Create: `macos/Sources/OwlBarCore/Runner.swift`
- Create: `macos/Tests/OwlBarCoreTests/RunnerTests.swift`

**Interfaces:**
- Consumes: `OwlFailure.notFound` (tâche 4).
- Produces:
  - `public protocol OwlRunning: Sendable { func run(_ arguments: [String]) async -> Result<Data, OwlFailure> }`
  - `public func locateOwl(home: String, isExecutable: (String) -> Bool) -> String?`
  - `public func childPath(home: String, inherited: String?) -> String`
  - `public struct ProcessRunner: OwlRunning` avec `public init()`.

- [ ] **Step 1: Écrire les tests qui échouent**

`macos/Tests/OwlBarCoreTests/RunnerTests.swift` :

```swift
import XCTest
@testable import OwlBarCore

final class RunnerTests: XCTestCase {
    func testCargoComesBeforeHomebrewAndUsrLocal() {
        let everywhere: (String) -> Bool = { _ in true }
        XCTAssertEqual(locateOwl(home: "/Users/me", isExecutable: everywhere), "/Users/me/.cargo/bin/owl")
    }

    func testTheFirstDirectoryHoldingOwlWins() {
        let onlyLocal: (String) -> Bool = { $0 == "/usr/local/bin/owl" }
        XCTAssertEqual(locateOwl(home: "/Users/me", isExecutable: onlyLocal), "/usr/local/bin/owl")
        let onlyBrew: (String) -> Bool = { $0 == "/opt/homebrew/bin/owl" }
        XCTAssertEqual(locateOwl(home: "/Users/me", isExecutable: onlyBrew), "/opt/homebrew/bin/owl")
    }

    func testNoOwlAnywhereGivesNothing() {
        XCTAssertNil(locateOwl(home: "/Users/me", isExecutable: { _ in false }))
    }

    func testTheChildPathPutsTheSearchDirectoriesFirst() {
        XCTAssertEqual(
            childPath(home: "/Users/me", inherited: "/usr/bin:/bin"),
            "/Users/me/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
        )
        XCTAssertEqual(
            childPath(home: "/Users/me", inherited: nil),
            "/Users/me/.cargo/bin:/opt/homebrew/bin:/usr/local/bin"
        )
    }
}
```

- [ ] **Step 2: Vérifier l'échec**

Run: `cd macos && swift test --filter RunnerTests`
Expected: échec de compilation, `locateOwl` et `childPath` introuvables.

- [ ] **Step 3: Écrire l'implémentation**

`macos/Sources/OwlBarCore/Runner.swift` :

```swift
// Recherche et lancement de `owl`. C'est le seul endroit de l'app qui lance
// un processus. Une app ouverte depuis le Finder ne voit pas le PATH du
// terminal : les dossiers d'installation habituels sont parcourus à la main,
// puis transmis à `owl` pour qu'il trouve `gh` à son tour.
import Foundation

public protocol OwlRunning: Sendable {
    /// Lance `owl` avec ces arguments et rend sa sortie standard brute.
    func run(_ arguments: [String]) async -> Result<Data, OwlFailure>
}

/// Dossiers parcourus, dans l'ordre. `~` désigne le dossier personnel.
let searchDirectories = ["~/.cargo/bin", "/opt/homebrew/bin", "/usr/local/bin"]

private func expanded(_ directory: String, home: String) -> String {
    directory.hasPrefix("~") ? home + directory.dropFirst() : directory
}

public func locateOwl(home: String, isExecutable: (String) -> Bool) -> String? {
    searchDirectories
        .map { expanded($0, home: home) + "/owl" }
        .first(where: isExecutable)
}

public func childPath(home: String, inherited: String?) -> String {
    let own = searchDirectories.map { expanded($0, home: home) }
    let rest = inherited.map { [$0] } ?? []
    return (own + rest).joined(separator: ":")
}

public struct ProcessRunner: OwlRunning {
    public init() {}

    public func run(_ arguments: [String]) async -> Result<Data, OwlFailure> {
        let home = NSHomeDirectory()
        guard let path = locateOwl(home: home, isExecutable: FileManager.default.isExecutableFile(atPath:)) else {
            return .failure(.notFound)
        }
        // Lecture bloquante hors du fil principal : le panneau reste
        // réactif pendant que `owl` interroge GitHub.
        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: path)
                process.arguments = arguments
                var environment = ProcessInfo.processInfo.environment
                environment["PATH"] = childPath(home: home, inherited: environment["PATH"])
                process.environment = environment
                let output = Pipe()
                process.standardOutput = output
                // La sortie d'erreur ne porte que l'aide d'usage : rien à
                // montrer, l'enveloppe absente suffit à signaler le problème.
                process.standardError = FileHandle.nullDevice
                do {
                    try process.run()
                } catch {
                    continuation.resume(returning: .failure(.notFound))
                    return
                }
                let data = output.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                continuation.resume(returning: .success(data))
            }
        }
    }
}
```

- [ ] **Step 4: Vérifier le passage**

Run: `cd macos && swift test`
Expected: tous les tests PASS.

- [ ] **Step 5: Commit**

```bash
git add macos/Sources/OwlBarCore/Runner.swift macos/Tests/OwlBarCoreTests/RunnerTests.swift
git commit -m "Recherche et lancement de owl depuis l'app

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Présentation, détection des PR prêtes et état du panneau

**Files:**
- Create: `macos/Sources/OwlBarCore/Presentation.swift`
- Create: `macos/Sources/OwlBarCore/ReadyTracker.swift`
- Create: `macos/Sources/OwlBarCore/BarState.swift`
- Create: `macos/Tests/OwlBarCoreTests/PresentationTests.swift`
- Create: `macos/Tests/OwlBarCoreTests/ReadyTrackerTests.swift`
- Create: `macos/Tests/OwlBarCoreTests/BarStateTests.swift`

**Interfaces:**
- Consumes: modèles et `decodeOwl` (tâche 4), `OwlRunning` (tâche 5).
- Produces :
  - `public enum Tone { green, red, yellow, gray }`, `public struct Glyph { symbol: String; tone: Tone }`
  - `public func checksGlyph(_:) -> Glyph`, `reviewGlyph(_:)`, `reviewLabel(_:) -> String`, `methodLabel(_:) -> String`, `shortAge(from:now:) -> String`
  - `public struct Counter { ready: Int; others: Int }`, `public func counter(_ prs: [PullRequest]) -> Counter`, `public func menuBarTitle(counter: Counter?, failed: Bool) -> String`
  - `public struct Row: Identifiable` (`id: PrKey`, `repo`, `number`, `age`, `target`, `title`, `checks: Glyph`, `review: Glyph`, `flags: [String]`), `public func rows(_ prs: [PullRequest], now: Date) -> [Row]`
  - `public struct DetailLine { glyph: Glyph?; text: String; note: String? }`, `public struct DetailSection { title: String; lines: [DetailLine] }`, `public func detailHeader(_ d: PrDetail) -> [String]`, `public func detailSections(_ d: PrDetail, now: Date) -> [DetailSection]`
  - `public func mergeChoices(_ pr: PullRequest) -> [MergeMethod]`, `public func preselected(_ choices: [MergeMethod], preferred: MergeMethod) -> MergeMethod?`, `public let noMergeMethodMessage: String`
  - `public let readyTitle = "Ready to merge"`, `public func readyBody(_ pr: PullRequest) -> String`
  - `public struct ReadyTracker { public init(); public mutating func update(_ prs: [PullRequest]) -> [PullRequest] }`
  - `public protocol Notifying: Sendable { func notify(title: String, body: String) }`
  - `@MainActor public final class BarState: ObservableObject` — voir l'étape 7.

- [ ] **Step 1: Tests de présentation**

`macos/Tests/OwlBarCoreTests/PresentationTests.swift` :

```swift
import XCTest
@testable import OwlBarCore

func recordedList() throws -> [PullRequest] {
    try decodeOwl(ListData.self, from: fixture("list")).get().pullRequests
}

final class PresentationTests: XCTestCase {
    let now = ISO8601DateFormatter().date(from: "2026-08-30T12:00:00Z")!

    func testTheAgeGoesFromMinutesToHoursToDays() {
        XCTAssertEqual(shortAge(from: now.addingTimeInterval(-59 * 60), now: now), "59m")
        XCTAssertEqual(shortAge(from: now.addingTimeInterval(-60 * 60), now: now), "1h")
        XCTAssertEqual(shortAge(from: now.addingTimeInterval(-25 * 3600), now: now), "1d")
    }

    func testADateInTheFutureGivesZero() {
        XCTAssertEqual(shortAge(from: now.addingTimeInterval(300), now: now), "0m")
    }

    func testEachStateHasTheGlyphOfTheTui() {
        XCTAssertEqual(checksGlyph(.success), Glyph(symbol: "✓", tone: .green))
        XCTAssertEqual(checksGlyph(.failure), Glyph(symbol: "✗", tone: .red))
        XCTAssertEqual(checksGlyph(.pending).tone, .yellow)
        XCTAssertEqual(checksGlyph(.none), Glyph(symbol: "·", tone: .gray))
        XCTAssertEqual(reviewGlyph(.approved), Glyph(symbol: "✓", tone: .green))
        XCTAssertEqual(reviewGlyph(.changesRequested), Glyph(symbol: "✗", tone: .red))
        XCTAssertEqual(reviewGlyph(.reviewRequired), Glyph(symbol: "●", tone: .yellow))
        XCTAssertEqual(reviewGlyph(.none), Glyph(symbol: "·", tone: .gray))
    }

    func testTheCounterSplitsReadyFromTheRest() throws {
        let prs = try recordedList()
        let ready = prs.filter { $0.mergeState == .clean }.count
        XCTAssertEqual(counter(prs), Counter(ready: ready, others: prs.count - ready))
    }

    func testTheMenuBarTitle() {
        XCTAssertEqual(menuBarTitle(counter: nil, failed: false), "🦉 …")
        XCTAssertEqual(menuBarTitle(counter: Counter(ready: 2, others: 5), failed: false), "🦉 2 · 5")
        XCTAssertEqual(menuBarTitle(counter: Counter(ready: 2, others: 5), failed: true), "🦉 ⚠")
    }

    func testARowCarriesTheColumnsOfTheTui() throws {
        let row = try XCTUnwrap(rows(recordedList(), now: now).first)
        XCTAssertEqual(row.repo, "moi/owl")
        XCTAssertEqual(row.number, "#42")
        XCTAssertEqual(row.target, "develop")
        XCTAssertEqual(row.title, "Ajoute la fenêtre de fusion")
        XCTAssertEqual(row.age, "2h")
        XCTAssertEqual(row.checks, checksGlyph(.success))
        XCTAssertEqual(row.flags, [])
    }

    func testDraftsAndConflictsAreFlagged() throws {
        let draft = try XCTUnwrap(rows(recordedList(), now: now).first { $0.number == "#7" })
        XCTAssertTrue(draft.flags.contains("draft"))
        let conflicting = try recordedList().filter { $0.mergeable == .conflicting }
        for row in rows(conflicting, now: now) {
            XCTAssertTrue(row.flags.contains("⚠ conflicts"))
        }
    }

    func testOnlyAllowedMethodsAreOfferedInOrder() throws {
        let pr = try recordedList()[0]
        XCTAssertEqual(mergeChoices(pr), [.squash, .rebase])
    }

    func testThePreferredMethodIsPreselectedWhenAllowed() {
        XCTAssertEqual(preselected([.squash, .rebase], preferred: .rebase), .rebase)
        XCTAssertEqual(preselected([.merge, .rebase], preferred: .squash), .merge)
        XCTAssertNil(preselected([], preferred: .squash))
    }

    func testTheNoMethodMessageMatchesTheTui() {
        XCTAssertEqual(noMergeMethodMessage, "No merge method allowed on this repository.")
    }

    func testMethodLabelsMatchTheTui() {
        XCTAssertEqual(methodLabel(.squash), "Squash and merge")
        XCTAssertEqual(methodLabel(.rebase), "Rebase and merge")
        XCTAssertEqual(methodLabel(.merge), "Create a merge commit")
    }

    func testTheDetailHasItsSectionsInOrder() throws {
        let detail = try decodeOwl(PrDetail.self, from: fixture("detail")).get()
        XCTAssertEqual(detailSections(detail, now: now).map(\.title), ["Checks", "Reviews", "Comments", "Files"])
        XCTAssertEqual(detailHeader(detail).last, "+214 −37")
    }

    func testTheNotificationText() throws {
        let pr = try recordedList()[0]
        XCTAssertEqual(readyTitle, "Ready to merge")
        XCTAssertEqual(readyBody(pr), "moi/owl #42 · Ajoute la fenêtre de fusion")
    }
}
```

Vérifier en lisant `Fixtures/list.json` que la PR `#7` est bien un brouillon et qu'au moins une PR est en conflit ; si aucune ne l'est, le second test ne vérifie rien : ajouter alors à `tests/fixtures/list.json` un nœud en `"mergeable": "CONFLICTING"`, relancer `OWL_UPDATE_FIXTURES=1 cargo test`, et corriger les tests Rust qui comptent 5 PR.

- [ ] **Step 2: Tests de détection des PR prêtes**

`macos/Tests/OwlBarCoreTests/ReadyTrackerTests.swift` :

```swift
import XCTest
@testable import OwlBarCore

/// Copie d'une PR enregistrée avec un autre verdict de fusion, en repassant
/// par le JSON : les modèles n'ont pas d'initialiseur public.
func withVerdict(_ pr: PullRequest, _ verdict: MergeVerdict, number: Int? = nil) throws -> PullRequest {
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    encoder.dateEncodingStrategy = .iso8601
    var object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: try encoder.encode(EncodablePR(pr))) as? [String: Any]
    )
    object["merge_state"] = verdict.rawValue
    if let number { object["number"] = number }
    let envelope: [String: Any] = ["version": 1, "ok": true, "data": object]
    return try decodeOwl(PullRequest.self, from: JSONSerialization.data(withJSONObject: envelope)).get()
}

final class ReadyTrackerTests: XCTestCase {
    func testNothingIsAnnouncedOnTheFirstList() throws {
        var tracker = ReadyTracker()
        XCTAssertEqual(tracker.update([try withVerdict(recordedList()[0], .clean)]), [])
    }

    func testAPullRequestBecomingReadyIsAnnouncedOnce() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .blocked)])
        let ready = try withVerdict(pr, .clean)
        XCTAssertEqual(tracker.update([ready]).map(\.key), [ready.key])
        XCTAssertEqual(tracker.update([ready]), [])
    }

    func testAnUnknownVerdictChangesNothing() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .clean)])
        _ = tracker.update([try withVerdict(pr, .unknown)])
        XCTAssertEqual(tracker.update([try withVerdict(pr, .clean)]), [])
    }

    func testBlockedThenReadyAgainIsAnnouncedAgain() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .clean)])
        _ = tracker.update([try withVerdict(pr, .blocked)])
        XCTAssertEqual(tracker.update([try withVerdict(pr, .clean)]).count, 1)
    }

    func testAPullRequestLeavingTheListIsForgotten() throws {
        let pr = try recordedList()[0]
        var tracker = ReadyTracker()
        _ = tracker.update([try withVerdict(pr, .clean)])
        _ = tracker.update([])
        XCTAssertEqual(tracker.update([try withVerdict(pr, .clean)]).count, 1)
    }
}
```

`withVerdict` a besoin d'encoder une `PullRequest`. Ajouter dans le même fichier de test un miroir encodable, pour ne pas rendre les modèles encodables pour les seuls tests :

```swift
struct EncodablePR: Encodable {
    let repo: String, number: Int, title: String, author: String, url: String
    let isDraft: Bool, checks: String, review: String, mergeable: String, mergeState: String
    let baseRef: String, headRef: String, updatedAt: Date, mergeMethods: [String]
    let checkRuns: [[String: String?]]

    init(_ pr: PullRequest) {
        repo = pr.repo; number = pr.number; title = pr.title; author = pr.author; url = pr.url
        isDraft = pr.isDraft; checks = pr.checks.rawValue; review = pr.review.rawValue
        mergeable = pr.mergeable.rawValue; mergeState = pr.mergeState.rawValue
        baseRef = pr.baseRef; headRef = pr.headRef; updatedAt = pr.updatedAt
        mergeMethods = pr.mergeMethods.map(\.rawValue)
        checkRuns = pr.checkRuns.map { ["name": $0.name, "state": $0.state.rawValue, "url": $0.url] }
    }
}
```

`decodeOwl(PullRequest.self, …)` lit l'objet sous `data` : c'est voulu, la fonction est générique.

- [ ] **Step 3: Tests de l'état**

`macos/Tests/OwlBarCoreTests/BarStateTests.swift` :

```swift
import XCTest
@testable import OwlBarCore

/// Faux `owl` : rend la réponse préparée pour la sous-commande, compte les
/// appels, et peut faire attendre la réponse.
final class FakeRunner: OwlRunning, @unchecked Sendable {
    var answers: [String: Result<Data, OwlFailure>] = [:]
    var calls: [[String]] = []
    var delay: UInt64 = 0

    func run(_ arguments: [String]) async -> Result<Data, OwlFailure> {
        calls.append(arguments)
        if delay > 0 { try? await Task.sleep(nanoseconds: delay) }
        return answers[arguments.joined(separator: " ")] ?? .failure(.unreadable)
    }
}

final class FakeNotifier: Notifying, @unchecked Sendable {
    var sent: [(String, String)] = []
    func notify(title: String, body: String) { sent.append((title, body)) }
}

func listAnswer(interval: Int = 60) throws -> Result<Data, OwlFailure> {
    var object = try XCTUnwrap(JSONSerialization.jsonObject(with: fixture("list")) as? [String: Any])
    var data = try XCTUnwrap(object["data"] as? [String: Any])
    data["refresh_interval"] = interval
    object["data"] = data
    return .success(try JSONSerialization.data(withJSONObject: object))
}

func errorAnswer(_ kind: String, _ message: String, resetAt: String? = nil) -> Result<Data, OwlFailure> {
    var error: [String: Any] = ["kind": kind, "message": message]
    if let resetAt { error["reset_at"] = resetAt }
    let object: [String: Any] = ["version": 1, "ok": false, "error": error]
    return .success(try! JSONSerialization.data(withJSONObject: object))
}

@MainActor
final class BarStateTests: XCTestCase {
    var runner: FakeRunner!
    var notifier: FakeNotifier!
    var clock = Date(timeIntervalSince1970: 1_800_000_000)
    var state: BarState!

    override func setUp() async throws {
        runner = FakeRunner()
        notifier = FakeNotifier()
        state = BarState(runner: runner, notifier: notifier, now: { [unowned self] in self.clock })
    }

    func testBeforeTheFirstListTheIconWaits() {
        XCTAssertEqual(state.title, "🦉 …")
    }

    func testAListFillsThePanelAndTheCounter() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        XCTAssertEqual(state.pullRequests.count, 5)
        XCTAssertNil(state.errorMessage)
        XCTAssertEqual(state.title, menuBarTitle(counter: counter(state.pullRequests), failed: false))
        XCTAssertEqual(state.lastRefresh, clock)
    }

    func testAFailureShowsOwlsMessageAndAlertsTheIcon() async throws {
        runner.answers["list"] = errorAnswer("token", "Not signed in to GitHub. Run `gh auth login`.")
        await state.refresh()
        XCTAssertEqual(state.errorMessage, "Not signed in to GitHub. Run `gh auth login`.")
        XCTAssertEqual(state.title, "🦉 ⚠")
    }

    func testAnUnreadableAnswerIsAnErrorNotACrash() async {
        runner.answers["list"] = .success(Data("Usage: owl […]".utf8))
        await state.refresh()
        XCTAssertEqual(state.errorMessage, "Unreadable answer from owl.")
    }

    func testASuccessAfterAFailureClearsTheAlert() async throws {
        runner.answers["list"] = .failure(.notFound)
        await state.refresh()
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        XCTAssertNil(state.errorMessage)
    }

    func testTwoRefreshesAtOnceLaunchOwlOnce() async throws {
        runner.answers["list"] = try listAnswer()
        runner.delay = 50_000_000
        async let first: Void = state.refresh()
        async let second: Void = state.refresh()
        _ = await (first, second)
        XCTAssertEqual(runner.calls.filter { $0 == ["list"] }.count, 1)
    }

    func testARateLimitPausesEveryRefreshUntilItsResetTime() async throws {
        let reset = ISO8601DateFormatter().string(from: clock.addingTimeInterval(600))
        runner.answers["list"] = errorAnswer("rate_limited", "Rate limit reached.", resetAt: reset)
        await state.refresh()
        runner.answers["list"] = try listAnswer()
        clock = clock.addingTimeInterval(300)
        await state.refresh()
        XCTAssertEqual(runner.calls.count, 1, "rien avant l'heure de reprise")
        clock = clock.addingTimeInterval(301)
        await state.refresh()
        XCTAssertEqual(runner.calls.count, 2)
        XCTAssertNil(state.errorMessage)
    }

    func testAZeroIntervalStopsAutomaticRefreshOnly() async throws {
        runner.answers["list"] = try listAnswer(interval: 0)
        await state.refresh()
        XCTAssertNil(state.nextRefreshDelay)
        await state.refresh()
        XCTAssertEqual(runner.calls.count, 2, "le bouton marche toujours")
    }

    func testTheIntervalComesFromOwl() async throws {
        XCTAssertEqual(state.nextRefreshDelay, 60, "valeur par défaut avant la première lecture")
        runner.answers["list"] = try listAnswer(interval: 120)
        await state.refresh()
        XCTAssertEqual(state.nextRefreshDelay, 120)
    }

    func testAPullRequestBecomingReadyIsNotified() async throws {
        let pr = try recordedList()[0]
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        XCTAssertTrue(notifier.sent.isEmpty, "rien à la première lecture")
        let blocked = try withVerdict(pr, .blocked, number: 4242)
        let ready = try withVerdict(pr, .clean, number: 4242)
        runner.answers["list"] = try listWith([blocked])
        await state.refresh()
        runner.answers["list"] = try listWith([ready])
        await state.refresh()
        XCTAssertEqual(notifier.sent.map(\.0), ["Ready to merge"])
        XCTAssertEqual(notifier.sent.map(\.1), [readyBody(ready)])
    }

    func testOpeningAPullRequestLoadsItsDetail() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        await state.open(key)
        XCTAssertEqual(state.openKey, key)
        guard case .loaded(let detail) = state.detail else { return XCTFail("détail attendu") }
        XCTAssertEqual(detail.summary.number, 42)
    }

    func testALateDetailForAnotherPullRequestIsIgnored() async throws {
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.delay = 50_000_000
        let first = Task { await state.open(PrKey(repo: "moi/owl", number: 42)) }
        await Task.yield()
        state.back()
        runner.delay = 0
        runner.answers["detail moi/notes 7"] = errorAnswer("not_found", "Pull request not found.")
        await state.open(PrKey(repo: "moi/notes", number: 7))
        await first.value
        XCTAssertEqual(state.openKey, PrKey(repo: "moi/notes", number: 7))
        XCTAssertEqual(state.detail, .failed("Pull request not found."))
    }

    func testAMergeSuccessGoesBackToAFreshList() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.answers["merge moi/owl 42 squash"] = .success(Data(#"{"version":1,"ok":true,"data":{}}"#.utf8))
        runner.answers["list"] = try listAnswer()
        await state.open(key)
        await state.merge(.squash)
        XCTAssertNil(state.openKey)
        XCTAssertEqual(state.mergeStatus, .idle)
        XCTAssertEqual(runner.calls.last, ["list"])
    }

    func testAMergeFailureShowsGithubsMessageVerbatim() async throws {
        let key = PrKey(repo: "moi/owl", number: 42)
        runner.answers["detail moi/owl 42"] = .success(try fixture("detail"))
        runner.answers["merge moi/owl 42 rebase"] = errorAnswer("api", "Base branch was modified. Review and try the merge again.")
        await state.open(key)
        await state.merge(.rebase)
        XCTAssertEqual(state.openKey, key)
        XCTAssertEqual(state.mergeStatus, .failed("Base branch was modified. Review and try the merge again."))
    }
}

func listWith(_ prs: [PullRequest]) throws -> Result<Data, OwlFailure> {
    let encoder = JSONEncoder()
    encoder.keyEncodingStrategy = .convertToSnakeCase
    encoder.dateEncodingStrategy = .iso8601
    let items = try prs.map { try JSONSerialization.jsonObject(with: encoder.encode(EncodablePR($0))) }
    let object: [String: Any] = [
        "version": 1, "ok": true,
        "data": ["pull_requests": items, "preferred_merge_method": "squash", "refresh_interval": 60],
    ]
    return .success(try JSONSerialization.data(withJSONObject: object))
}
```

- [ ] **Step 4: Vérifier l'échec**

Run: `cd macos && swift test`
Expected: échec de compilation, `shortAge`, `ReadyTracker`, `BarState`… introuvables.

- [ ] **Step 5: Écrire la présentation**

`macos/Sources/OwlBarCore/Presentation.swift` :

```swift
// Tout ce que le panneau affiche, déjà décidé : pictogrammes, libellés,
// âges, colonnes. Les vues ne choisissent rien. Les choix suivent ceux de la
// TUI (`src/app/render.rs`) pour qu'on retrouve les mêmes repères.
import Foundation

public enum Tone: Sendable { case green, red, yellow, gray }

public struct Glyph: Equatable, Sendable {
    public let symbol: String
    public let tone: Tone
}

public func checksGlyph(_ state: ChecksState) -> Glyph {
    switch state {
    case .success: Glyph(symbol: "✓", tone: .green)
    case .failure: Glyph(symbol: "✗", tone: .red)
    // Pas d'animation dans un menu : un cercle vide dit « en cours ».
    case .pending: Glyph(symbol: "◌", tone: .yellow)
    case .none: Glyph(symbol: "·", tone: .gray)
    }
}

public func reviewGlyph(_ state: ReviewState) -> Glyph {
    switch state {
    case .approved: Glyph(symbol: "✓", tone: .green)
    case .changesRequested: Glyph(symbol: "✗", tone: .red)
    case .reviewRequired: Glyph(symbol: "●", tone: .yellow)
    case .none: Glyph(symbol: "·", tone: .gray)
    }
}

public func reviewLabel(_ state: ReviewState) -> String {
    switch state {
    case .approved: "approved"
    case .changesRequested: "changes requested"
    case .reviewRequired: "review required"
    case .none: "nothing to report"
    }
}

public func methodLabel(_ method: MergeMethod) -> String {
    switch method {
    case .squash: "Squash and merge"
    case .rebase: "Rebase and merge"
    case .merge: "Create a merge commit"
    }
}

/// Minutes, puis heures, puis jours. Une date future donne zéro.
public func shortAge(from date: Date, now: Date) -> String {
    let minutes = max(0, Int(now.timeIntervalSince(date) / 60))
    if minutes < 60 { return "\(minutes)m" }
    let hours = minutes / 60
    if hours < 24 { return "\(hours)h" }
    return "\(hours / 24)d"
}

public struct Counter: Equatable, Sendable {
    public let ready: Int
    public let others: Int
}

/// « Prête » : GitHub dit que plus rien ne bloque. Tout le reste, brouillons
/// compris, compte dans le second nombre.
public func counter(_ prs: [PullRequest]) -> Counter {
    let ready = prs.filter { $0.mergeState == .clean }.count
    return Counter(ready: ready, others: prs.count - ready)
}

public func menuBarTitle(counter: Counter?, failed: Bool) -> String {
    if failed { return "🦉 ⚠" }
    guard let counter else { return "🦉 …" }
    return "🦉 \(counter.ready) · \(counter.others)"
}

public struct Row: Equatable, Identifiable, Sendable {
    public let id: PrKey
    public let repo: String
    public let number: String
    public let age: String
    public let target: String
    public let title: String
    public let checks: Glyph
    public let review: Glyph
    public let flags: [String]
}

public func rows(_ prs: [PullRequest], now: Date) -> [Row] {
    prs.map { pr in
        var flags: [String] = []
        if pr.isDraft { flags.append("draft") }
        if pr.mergeable == .conflicting { flags.append("⚠ conflicts") }
        return Row(
            id: pr.key,
            repo: pr.repo,
            number: "#\(pr.number)",
            age: shortAge(from: pr.updatedAt, now: now),
            target: pr.baseRef,
            title: pr.title,
            checks: checksGlyph(pr.checks),
            review: reviewGlyph(pr.review),
            flags: flags
        )
    }
}

public struct DetailLine: Equatable, Sendable {
    public let glyph: Glyph?
    public let text: String
    public let note: String?
}

public struct DetailSection: Equatable, Sendable {
    public let title: String
    public let lines: [DetailLine]
}

/// Lignes d'en-tête du détail, sous le titre.
public func detailHeader(_ detail: PrDetail) -> [String] {
    let pr = detail.summary
    return [
        "\(pr.repo) #\(pr.number) · by \(pr.author)",
        "\(pr.headRef) → \(pr.baseRef)",
        "+\(detail.additions) −\(detail.deletions)",
    ]
}

public func detailSections(_ detail: PrDetail, now: Date) -> [DetailSection] {
    [
        DetailSection(title: "Checks", lines: detail.checks.map {
            DetailLine(glyph: checksGlyph($0.state), text: $0.name, note: nil)
        }),
        DetailSection(title: "Reviews", lines: detail.reviews.map {
            DetailLine(
                glyph: reviewGlyph($0.state),
                text: "\($0.author) · \(reviewLabel($0.state)) · \(shortAge(from: $0.submittedAt, now: now))",
                note: $0.body.isEmpty ? nil : $0.body
            )
        }),
        DetailSection(title: "Comments", lines: detail.comments.map {
            DetailLine(glyph: nil, text: "\($0.author) · \(shortAge(from: $0.createdAt, now: now))", note: $0.body)
        }),
        DetailSection(title: "Files", lines: detail.files.map {
            DetailLine(glyph: nil, text: $0.path, note: "+\($0.additions) −\($0.deletions)")
        }),
    ]
}

public let noMergeMethodMessage = "No merge method allowed on this repository."

public func mergeChoices(_ pr: PullRequest) -> [MergeMethod] {
    MergeMethod.allCases.filter { pr.mergeMethods.contains($0) }
}

public func preselected(_ choices: [MergeMethod], preferred: MergeMethod) -> MergeMethod? {
    choices.contains(preferred) ? preferred : choices.first
}

public let readyTitle = "Ready to merge"

public func readyBody(_ pr: PullRequest) -> String {
    "\(pr.repo) #\(pr.number) · \(pr.title)"
}
```

Les `switch` utilisés comme expressions demandent Swift 5.9 ou plus : c'est le cas.

- [ ] **Step 6: Écrire la détection des PR prêtes**

`macos/Sources/OwlBarCore/ReadyTracker.swift` :

```swift
// Annonce des pull requests devenues prêtes, avec la règle de la TUI
// (`App::announce_ready`) : rien à la première liste, une annonce par
// passage, un verdict inconnu ne change rien, une PR sortie de la liste est
// oubliée.
public struct ReadyTracker: Sendable {
    private var announced: Set<PrKey>?

    public init() {}

    /// Rend les pull requests à annoncer, dans l'ordre de la liste.
    public mutating func update(_ prs: [PullRequest]) -> [PullRequest] {
        guard var announced else {
            announced = Set(prs.filter { $0.mergeState == .clean }.map(\.key))
            return []
        }
        let present = Set(prs.map(\.key))
        announced.formIntersection(present)
        var newlyReady: [PullRequest] = []
        for pr in prs {
            switch pr.mergeState {
            case .clean:
                if announced.insert(pr.key).inserted { newlyReady.append(pr) }
            case .blocked:
                announced.remove(pr.key)
            case .unknown:
                break
            }
        }
        self.announced = announced
        return newlyReady
    }
}
```

- [ ] **Step 7: Écrire l'état**

`macos/Sources/OwlBarCore/BarState.swift` :

```swift
// État du panneau. Il demande à `owl` et décide de ce qui s'affiche ; il ne
// lance aucun processus lui-même (c'est le travail de `OwlRunning`) et ne
// dessine rien.
import Foundation

public protocol Notifying: Sendable {
    func notify(title: String, body: String)
}

public enum DetailStatus: Equatable, Sendable {
    case loading
    case loaded(PrDetail)
    case failed(String)
}

public enum MergeStatus: Equatable, Sendable {
    case idle
    case running
    case failed(String)
}

@MainActor
public final class BarState: ObservableObject {
    @Published public private(set) var pullRequests: [PullRequest] = []
    @Published public private(set) var loaded = false
    @Published public private(set) var errorMessage: String?
    @Published public private(set) var isRefreshing = false
    @Published public private(set) var lastRefresh: Date?
    @Published public private(set) var openKey: PrKey?
    @Published public private(set) var detail: DetailStatus = .loading
    @Published public private(set) var mergeStatus: MergeStatus = .idle
    public private(set) var preferredMethod: MergeMethod = .squash
    private var refreshInterval = 60
    private var pausedUntil: Date?
    private var tracker = ReadyTracker()

    private let runner: OwlRunning
    private let notifier: Notifying
    private let now: () -> Date

    public init(runner: OwlRunning, notifier: Notifying, now: @escaping () -> Date = Date.init) {
        self.runner = runner
        self.notifier = notifier
        self.now = now
    }

    public var title: String {
        menuBarTitle(counter: loaded ? counter(pullRequests) : nil, failed: errorMessage != nil)
    }

    public var rows: [Row] { OwlBarCore.rows(pullRequests, now: now()) }

    /// « Updated 3m ago », ou « Updating… » pendant une lecture.
    public var refreshLabel: String {
        if isRefreshing { return "Updating…" }
        guard let lastRefresh else { return "" }
        return "Updated \(shortAge(from: lastRefresh, now: now())) ago"
    }

    /// Délai avant la prochaine lecture automatique ; `nil` quand la
    /// configuration la désactive.
    public var nextRefreshDelay: TimeInterval? {
        refreshInterval > 0 ? TimeInterval(refreshInterval) : nil
    }

    public var openPullRequest: PullRequest? {
        guard let openKey else { return nil }
        if case .loaded(let detail) = detail, detail.summary.key == openKey { return detail.summary }
        return pullRequests.first { $0.key == openKey }
    }

    public var detailHeaderLines: [String] {
        if case .loaded(let detail) = detail { return detailHeader(detail) }
        return []
    }

    public var detailSectionList: [DetailSection] {
        if case .loaded(let detail) = detail { return detailSections(detail, now: now()) }
        return []
    }

    /// Méthodes à proposer pour la PR ouverte, et celle présélectionnée.
    public var mergeOptions: (choices: [MergeMethod], selected: MergeMethod?) {
        guard let pr = openPullRequest else { return ([], nil) }
        let choices = mergeChoices(pr)
        return (choices, preselected(choices, preferred: preferredMethod))
    }

    public func refresh() async {
        guard !isRefreshing else { return }
        if let pausedUntil, now() < pausedUntil { return }
        isRefreshing = true
        defer { isRefreshing = false }

        let result = await runner.run(["list"]).flatMap { decodeOwl(ListData.self, from: $0) }
        switch result {
        case .success(let list):
            pullRequests = list.pullRequests
            preferredMethod = list.preferredMergeMethod
            refreshInterval = list.refreshInterval
            pausedUntil = nil
            errorMessage = nil
            loaded = true
            lastRefresh = now()
            for pr in tracker.update(list.pullRequests) {
                notifier.notify(title: readyTitle, body: readyBody(pr))
            }
        case .failure(let failure):
            errorMessage = failure.message
            if failure.kind == "rate_limited" { pausedUntil = failure.resetAt }
        }
    }

    public func open(_ key: PrKey) async {
        openKey = key
        detail = .loading
        mergeStatus = .idle
        let result = await runner.run(["detail", key.repo, String(key.number)])
            .flatMap { decodeOwl(PrDetail.self, from: $0) }
        // Retour à la liste ou autre PR ouverte entre-temps : cette réponse
        // ne concerne plus l'écran.
        guard openKey == key else { return }
        switch result {
        case .success(let value): detail = .loaded(value)
        case .failure(let failure): detail = .failed(failure.message)
        }
    }

    public func back() {
        openKey = nil
        mergeStatus = .idle
    }

    public func merge(_ method: MergeMethod) async {
        guard let key = openKey, mergeStatus != .running else { return }
        mergeStatus = .running
        let result = await runner.run(["merge", key.repo, String(key.number), method.rawValue])
            .flatMap { decodeOwl(MergeData.self, from: $0) }
        switch result {
        case .success:
            mergeStatus = .idle
            if openKey == key { openKey = nil }
            await refresh()
        case .failure(let failure):
            mergeStatus = .failed(failure.message)
        }
    }
}
```

- [ ] **Step 8: Vérifier le passage**

Run: `cd macos && swift build && swift test`
Expected: tous les tests PASS, sans avertissement de concurrence bloquant (le mode de langage 5 les laisse en avertissements ; ne pas passer en mode 6 dans ce plan).

Si `testALateDetailForAnotherPullRequestIsIgnored` est instable, c'est que le premier `open` n'a pas encore appelé le faux `owl` au moment du `back()` : remplacer `await Task.yield()` par une boucle `while runner.calls.isEmpty { await Task.yield() }`.

- [ ] **Step 9: Commit**

```bash
git add macos/Sources/OwlBarCore macos/Tests/OwlBarCoreTests
git commit -m "Présentation, annonces et état du panneau de la barre des menus

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: L'app, ses vues, son installation et la documentation

**Files:**
- Replace: `macos/Sources/OwlBar/OwlBarApp.swift`
- Create: `macos/Sources/OwlBar/PanelView.swift`, `ListView.swift`, `DetailView.swift`, `SystemNotifier.swift`, `LaunchAtLogin.swift`
- Create: `macos/Resources/Info.plist`, `macos/build.sh`
- Modify: `CLAUDE.md` (section Commandes), `DESCRIPTION.md` (nouvelle section)

**Interfaces:**
- Consumes: `BarState` et tout `OwlBarCore` (tâche 6), `ProcessRunner` (tâche 5).
- Produces: `macos/build/Owl.app`, installée dans `~/Applications/Owl.app`.

Rien dans ces vues ne se teste automatiquement. La règle reste : aucune décision dans une vue. Une vue lit `state` et appelle ses méthodes ; elle ne compose pas de texte autre que les intitulés fixes des boutons.

- [ ] **Step 1: Notifications et ouverture de session**

`macos/Sources/OwlBar/SystemNotifier.swift` :

```swift
// Notifications du système. Un refus de l'utilisateur ou un lancement hors
// du paquet `.app` (par `swift run`) font simplement taire l'app.
import Foundation
import OwlBarCore
import UserNotifications

struct SystemNotifier: Notifying {
    static func requestPermission() {
        guard Bundle.main.bundleIdentifier != nil else { return }
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    func notify(title: String, body: String) {
        // Sans identifiant de paquet, `UNUserNotificationCenter` arrête le
        // programme : on ne tente rien.
        guard Bundle.main.bundleIdentifier != nil else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
```

`macos/Sources/OwlBar/LaunchAtLogin.swift` :

```swift
// Lancement à l'ouverture de session. Inscrit une seule fois d'office, au
// premier lancement ; ensuite, seule la case du panneau décide.
import Foundation
import ServiceManagement

enum LaunchAtLogin {
    private static let registeredKey = "launchAtLoginRegisteredOnce"

    static var isEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    static func registerOnFirstLaunch() {
        let defaults = UserDefaults.standard
        guard Bundle.main.bundleIdentifier != nil, !defaults.bool(forKey: registeredKey) else { return }
        try? SMAppService.mainApp.register()
        defaults.set(true, forKey: registeredKey)
    }

    static func set(_ enabled: Bool) {
        if enabled {
            try? SMAppService.mainApp.register()
        } else {
            try? SMAppService.mainApp.unregister()
        }
    }
}
```

- [ ] **Step 2: Point d'entrée**

Remplacer `macos/Sources/OwlBar/OwlBarApp.swift` :

```swift
// App de la barre des menus : une icône avec le compteur, un panneau au
// clic. Pas d'icône dans le Dock (`LSUIElement` dans `Info.plist`).
import AppKit
import OwlBarCore
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let state = BarState(runner: ProcessRunner(), notifier: SystemNotifier())
    private var loop: Task<Void, Never>?

    func applicationDidFinishLaunching(_ notification: Notification) {
        SystemNotifier.requestPermission()
        LaunchAtLogin.registerOnFirstLaunch()
        loop = Task { [state] in
            await state.refresh()
            // Un intervalle à zéro arrête la boucle : seul le bouton du
            // panneau rafraîchit alors.
            while let delay = state.nextRefreshDelay {
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                await state.refresh()
            }
        }
    }
}

@main
struct OwlBarApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        MenuBarExtra {
            PanelView(state: delegate.state)
        } label: {
            MenuBarLabel(state: delegate.state)
        }
        .menuBarExtraStyle(.window)
    }
}

struct MenuBarLabel: View {
    @ObservedObject var state: BarState

    var body: some View {
        Text(state.title)
    }
}
```

- [ ] **Step 3: Le panneau et la liste**

`macos/Sources/OwlBar/PanelView.swift` :

```swift
import OwlBarCore
import SwiftUI

struct PanelView: View {
    @ObservedObject var state: BarState

    var body: some View {
        Group {
            if state.openKey != nil {
                DetailView(state: state)
                    .transition(.move(edge: .trailing))
            } else {
                ListView(state: state)
                    .transition(.move(edge: .leading))
            }
        }
        .animation(.easeInOut(duration: 0.2), value: state.openKey)
        .frame(width: 480, height: 520)
    }
}

extension Tone {
    var color: Color {
        switch self {
        case .green: .green
        case .red: .red
        case .yellow: .yellow
        case .gray: .secondary
        }
    }
}

struct GlyphView: View {
    let glyph: Glyph

    var body: some View {
        Text(glyph.symbol)
            .foregroundStyle(glyph.tone.color)
            .frame(width: 14)
    }
}
```

`macos/Sources/OwlBar/ListView.swift` :

```swift
import OwlBarCore
import SwiftUI

struct ListView: View {
    @ObservedObject var state: BarState
    @State private var launchAtLogin = LaunchAtLogin.isEnabled

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(state.refreshLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    Task { await state.refresh() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
                .disabled(state.isRefreshing)
                .keyboardShortcut("r")
            }
            .padding(10)

            Divider()

            if let message = state.errorMessage {
                Text(message)
                    .textSelection(.enabled)
                    .multilineTextAlignment(.center)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(state.rows) { row in
                            Button {
                                Task { await state.open(row.id) }
                            } label: {
                                RowView(row: row)
                            }
                            .buttonStyle(.plain)
                            Divider()
                        }
                    }
                }
            }

            Divider()

            HStack {
                Toggle("Launch at login", isOn: $launchAtLogin)
                    .toggleStyle(.checkbox)
                    .onChange(of: launchAtLogin) { _, enabled in
                        LaunchAtLogin.set(enabled)
                    }
                Spacer()
                Button("Quit") { NSApplication.shared.terminate(nil) }
                    .keyboardShortcut("q")
            }
            .padding(10)
        }
    }
}

struct RowView: View {
    let row: Row

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                GlyphView(glyph: row.checks)
                GlyphView(glyph: row.review)
                Text(row.repo).foregroundStyle(.cyan)
                Text(row.number).foregroundStyle(.secondary)
                Text(row.age).foregroundStyle(.secondary)
                Text(row.target).foregroundStyle(.purple)
                ForEach(row.flags, id: \.self) { flag in
                    Text(flag).font(.caption).foregroundStyle(.orange)
                }
                Spacer()
            }
            .font(.system(.body, design: .monospaced))
            Text(row.title)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.leading, 40)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}
```

- [ ] **Step 4: Le détail et la fusion**

`macos/Sources/OwlBar/DetailView.swift` :

```swift
import OwlBarCore
import SwiftUI

struct DetailView: View {
    @ObservedObject var state: BarState

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Button {
                    state.back()
                } label: {
                    Image(systemName: "chevron.left")
                }
                .buttonStyle(.borderless)
                .keyboardShortcut(.leftArrow, modifiers: [])
                Text(state.openPullRequest?.title ?? "")
                    .font(.headline)
                    .lineLimit(2)
                Spacer()
                if let url = state.openPullRequest.flatMap({ URL(string: $0.url) }) {
                    Link("Open on GitHub", destination: url)
                }
            }
            .padding(10)

            Divider()

            switch state.detail {
            case .loading:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let message):
                Text(message)
                    .textSelection(.enabled)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .loaded(let detail):
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(state.detailHeaderLines, id: \.self) { line in
                            Text(line).foregroundStyle(.secondary)
                        }
                        if !detail.body.isEmpty {
                            Text(detail.body).textSelection(.enabled)
                        }
                        ForEach(state.detailSectionList, id: \.title) { section in
                            if !section.lines.isEmpty {
                                Text(section.title).font(.headline)
                                ForEach(Array(section.lines.enumerated()), id: \.offset) { _, line in
                                    LineView(line: line)
                                }
                            }
                        }
                    }
                    .padding(10)
                }
            }

            Divider()
            MergeBar(state: state).padding(10)
        }
    }
}

struct LineView: View {
    let line: DetailLine

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                if let glyph = line.glyph { GlyphView(glyph: glyph) }
                Text(line.text)
            }
            if let note = line.note {
                Text(note).foregroundStyle(.secondary).textSelection(.enabled)
            }
        }
    }
}

struct MergeBar: View {
    @ObservedObject var state: BarState
    @State private var chosen: MergeMethod?
    @State private var confirming = false

    var body: some View {
        let options = state.mergeOptions
        VStack(alignment: .leading, spacing: 6) {
            if options.choices.isEmpty {
                Text(noMergeMethodMessage).foregroundStyle(.secondary)
            } else if state.mergeStatus == .running {
                ProgressView().controlSize(.small)
            } else if confirming, let method = chosen ?? options.selected {
                HStack {
                    Button(methodLabel(method)) {
                        confirming = false
                        Task { await state.merge(method) }
                    }
                    .keyboardShortcut(.defaultAction)
                    Button("Cancel") { confirming = false }
                        .keyboardShortcut(.cancelAction)
                }
            } else {
                HStack {
                    if options.choices.count > 1 {
                        Picker("", selection: Binding(
                            get: { chosen ?? options.selected ?? options.choices[0] },
                            set: { chosen = $0 }
                        )) {
                            ForEach(options.choices, id: \.self) { method in
                                Text(methodLabel(method)).tag(method)
                            }
                        }
                        .labelsHidden()
                        .fixedSize()
                    }
                    Button("Merge") { confirming = true }
                        .keyboardShortcut("m", modifiers: [])
                }
            }
            if case .failed(let message) = state.mergeStatus {
                Text(message).foregroundStyle(.red).textSelection(.enabled)
            }
        }
        .onChange(of: state.openKey) { _, _ in
            chosen = nil
            confirming = false
        }
    }
}
```

- [ ] **Step 5: Paquet `.app` et installation**

`macos/Resources/Info.plist` :

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>fr.gkfa.owl-bar</string>
    <key>CFBundleName</key>
    <string>Owl</string>
    <key>CFBundleExecutable</key>
    <string>OwlBar</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>LSUIElement</key>
    <true/>
</dict>
</plist>
```

`macos/build.sh` (rendre exécutable : `chmod +x macos/build.sh`) :

```sh
#!/bin/sh
# Compile l'app de la barre des menus, assemble Owl.app, la signe localement
# et l'installe dans ~/Applications.
set -eu
cd "$(dirname "$0")"

swift build -c release
binaries="$(swift build -c release --show-bin-path)"

app="build/Owl.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$binaries/OwlBar" "$app/Contents/MacOS/OwlBar"
cp Resources/Info.plist "$app/Contents/Info.plist"
codesign --force --sign - "$app"

mkdir -p "$HOME/Applications"
rm -rf "$HOME/Applications/Owl.app"
cp -R "$app" "$HOME/Applications/"
echo "Installed: $HOME/Applications/Owl.app"
```

- [ ] **Step 6: Documentation**

Dans `CLAUDE.md`, section « Commandes », ajouter après le bloc existant :

````markdown
L'app de la barre des menus vit dans `macos/` :

```bash
cd macos && swift build
cd macos && swift test
macos/build.sh         # construit et installe ~/Applications/Owl.app
```

Les deux premières doivent passer dès qu'un fichier de `macos/` ou de
`src/cli/` change. `OWL_UPDATE_FIXTURES=1 cargo test` régénère les exemples
JSON lus par les tests de l'app.
````

Dans la section « Règles d'architecture » de `CLAUDE.md`, ajouter :

```markdown
- `cli` dépend de `model`, `filter`, `config`, `token` et `github`, jamais de
  `app` ni `ui`. Son JSON est le contrat de l'app de la barre des menus.
- Dans `macos/`, `OwlBarCore` décide, `OwlBar` dessine. Une vue ne choisit ni
  pictogramme, ni texte, ni format.
```

Dans `DESCRIPTION.md`, avant « Ce que owl ne fait pas », ajouter :

```markdown
## Dans la barre des menus

Une petite app macOS, `Owl.app`, garde `owl` sous la main hors du terminal.
Une chouette dans la barre des menus affiche le nombre de PR prêtes à
fusionner et le nombre des autres. Un clic ouvre un panneau avec la même
liste, le détail d'une PR et la fusion. L'app interroge `owl` en coulisses,
avec les sous-commandes `owl list`, `owl detail` et `owl merge`, qui
répondent en JSON. Elle se lance à l'ouverture de session et prévient quand
une PR devient fusionnable.
```

- [ ] **Step 7: Vérifications automatiques**

Run: `cd macos && swift build && swift test`
Expected: PASS.

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: PASS.

- [ ] **Step 8: Essai réel**

Run: `cargo install --path . && macos/build.sh && open ~/Applications/Owl.app`

Vérifier à l'œil, et noter chaque point dans le rapport :
1. La chouette et le compteur apparaissent dans la barre des menus en quelques secondes.
2. Le clic ouvre le panneau avec la liste ; les colonnes ressemblent à celles de la TUI.
3. Un clic sur une PR glisse vers le détail ; la flèche retour revient.
4. « Open on GitHub » ouvre le navigateur.
5. Le bouton « Merge » montre les méthodes autorisées et attend une confirmation. **Ne pas confirmer** sur une vraie PR : annuler.
6. Rebaptiser temporairement `~/.cargo/bin/owl` puis rafraîchir : `🦉 ⚠` et le message « owl not found… ». Remettre le fichier en place.

Si une étape échoue, la corriger dans cette tâche ; si la correction touche `OwlBarCore`, ajouter le test qui l'aurait vue.

- [ ] **Step 9: Commit et pull request**

```bash
git add macos CLAUDE.md DESCRIPTION.md
git commit -m "App de la barre des menus : vues, installation et documentation

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
git push -u origin feature/icone-barre-menus
gh pr create --base develop --title "owl dans la barre des menus" --body "$(cat <<'EOF'
Ajoute les sous-commandes JSON `owl list`, `owl detail` et `owl merge`, et une app macOS (`macos/`) qui vit dans la barre des menus : compteur de PR prêtes et en cours, panneau avec liste, détail et fusion, notifications, lancement à l'ouverture de session.

Spec : `docs/superpowers/specs/2026-10-13-barre-des-menus-design.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Rapport final : le lien de la PR, puis seulement ce qui reste à vérifier à la main ou ce qui n'a pas pu l'être.
