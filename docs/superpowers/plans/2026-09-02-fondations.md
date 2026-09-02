# Fondations de owl — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Poser le squelette de `owl` : un binaire Rust qui résout un jeton, lit ses réglages, ouvre un écran plein terminal sur une boucle d'événements asynchrone, et le referme proprement sur « q » comme sur panique.

**Architecture:** Six modules aux dépendances à sens unique. `token.rs` et `config.rs` sont des fonctions pures testables, appelées **avant** toute prise de contrôle du terminal, pour que leurs messages d'erreur ne soient jamais avalés par l'écran alterné. `main.rs` tient la boucle : trois producteurs (clavier, minuteur, résultats réseau) alimentent une file `tokio::mpsc`, chaque événement passe à `app::App::handle` qui renvoie des commandes que `main` exécute, puis l'écran est redessiné. `app` ne touche ni au réseau ni au terminal ; `ui` ne fait que lire `app` et dessiner. Les demandes réseau portent un numéro de génération, ce qui permet d'ignorer une réponse périmée.

**Tech Stack:** Rust édition 2021, `ratatui` + `crossterm` (écran et clavier), `tokio` (exécuteur), `reqwest` + `rustls` (HTTP), `serde` / `serde_json` / `toml` (sérialisation et réglages), `thiserror` + `anyhow` (erreurs), `directories` (chemins), `chrono` (dates), `open` (navigateur).

**Spec:** `docs/specs/00-fondations.md` (contexte transverse : `docs/specs/05-erreurs-et-tests.md` pour les messages d'erreur de démarrage, `docs/specs/02-filtres.md` pour la clé `filters`)

## Global Constraints

- Rust, édition 2021. Binaire unique nommé `owl`, aucune sous-commande.
- Bibliothèques imposées, aucune autre dépendance de production : `ratatui`, `crossterm`, `tokio`, `reqwest` (avec `rustls`, jamais OpenSSL), `serde`, `serde_json`, `toml`, `thiserror` (erreurs du domaine), `anyhow` (dans le binaire), `directories`, `chrono`, `open`.
- Arborescence des modules exactement celle de la spec : `main.rs`, `config.rs`, `token.rs`, `github/{mod,queries,dto}.rs`, `model.rs`, `filter.rs`, `app.rs`, `ui/{mod,list,detail,merge}.rs`. Aucun autre fichier source.
- Dépendances à sens unique : `model` et `filter` ne dépendent que de la bibliothèque standard et de `serde` ; `github` dépend de `model` et `filter`, jamais de `app` ni `ui` ; `app` dépend de `model` et `filter` et ne fait aucun appel réseau ; `ui` lit `app` en lecture seule et une fonction de dessin ne modifie jamais l'état.
- Le jeton n'est jamais écrit dans un fichier, ni journalisé, ni affiché.
- Ordre de résolution du jeton, arrêt au premier trouvé : `OWL_TOKEN`, puis `GITHUB_TOKEN`, puis la sortie de `gh auth token`.
- Réglages : `~/.config/owl/config.toml`, fichier optionnel, clé inconnue ignorée sans erreur, valeur invalide = arrêt avec un message précisant la clé fautive.
- Valeurs par défaut : `filters = ["author:@me", "is:open"]`, `refresh_interval = 60`, `preferred_merge_method = "squash"`, `page_size = 50` (bornes 1 à 100).
- Messages d'erreur de démarrage, au mot près (`docs/specs/05-erreurs-et-tests.md`) :
  - `gh` absent du `PATH` et aucune variable de jeton → « owl a besoin de gh. Installe-le, puis lance `gh auth login`. »
  - `gh` présent mais non connecté → « Non connecté à GitHub. Lance `gh auth login`. »
  - Fichier de réglages illisible ou invalide → « Réglages invalides dans <chemin> : <clé fautive>. »
  - Liste de filtres vide → « Aucun filtre actif : la recherche ramènerait tout GitHub. »
- Toute erreur de démarrage : message sur la sortie d'erreur, code de sortie non nul, sans prise de contrôle du terminal.
- Le mode brut et l'écran alterné sont restaurés par un garde de portée **doublé** d'un crochet de panique.
- Le projet est en français : messages affichés, commentaires, messages de commit. Les identifiants du code restent en anglais.
- `cargo build`, `cargo test`, `cargo clippy -- -D warnings` et `cargo fmt --check` doivent passer à la fin de chaque tâche. Aucune n'est optionnelle.
- Branche de travail : `feat/fondations`, créée depuis `develop` à la première étape de la tâche 1. Ne jamais travailler directement sur `develop`.

## Décisions mises de côté

Toute décision **reportée** pendant l'exécution se consigne dans `docs/suivi/DETTE.md` : une ligne, ce qui est en suspens et pourquoi. Uniquement ce qui est reporté et critique pour la suite — pas les décisions prises, pas les idées d'amélioration, pas de sur-conception. Si rien n'est reporté, le fichier n'existe pas.

## Structure des fichiers

| Fichier | Responsabilité | Tâche |
|---|---|---|
| `Cargo.toml` | Métadonnées et dépendances épinglées | 1 |
| `src/token.rs` | Résolution du jeton, type `Token` qui masque son contenu | 1 |
| `src/config.rs` | Lecture et validation de `config.toml`, valeurs par défaut | 2 |
| `src/main.rs` | Démarrage, garde du terminal, crochet de panique, boucle d'événements, exécution des commandes | 3 puis 5 |
| `src/model.rs` | Types métier (`PullRequest` minimal) | 4 |
| `src/filter.rs` | Module documenté, vide au stade fondations | 4 |
| `src/github/mod.rs` | Fonction de récupération, bouchon au stade fondations | 4 |
| `src/github/queries.rs` | Module documenté, vide au stade fondations | 4 |
| `src/github/dto.rs` | Module documenté, vide au stade fondations | 4 |
| `src/app.rs` | État, `handle(Event) -> Vec<Command>`, générations | 4 |
| `src/ui/mod.rs` | Aiguillage de dessin | 5 |
| `src/ui/list.rs` | Dessin de la liste et de la barre d'état | 5 |
| `src/ui/detail.rs` | Module documenté, vide au stade fondations | 5 |
| `src/ui/merge.rs` | Module documenté, vide au stade fondations | 5 |
| `tests/startup.rs` | Tests d'intégration sur le binaire (messages et codes de sortie) | 3 |

Les modules laissés vides existent parce que la spec fixe l'arborescence. Un module vide compile sans avertissement ; il sera rempli par les specs 01 à 04.

---

### Task 1: Squelette du projet et résolution du jeton

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `src/main.rs`
- Create: `src/token.rs`

**Interfaces:**
- Consumes: rien.
- Produces:
  - `token::Token` — enveloppe opaque autour de la chaîne du jeton. `Token::expose(&self) -> &str`. Son `Debug` affiche `Token(masqué)`.
  - `token::TokenError` — `enum` `thiserror` dont l'affichage donne les messages exacts de la spec. Variantes : `GhMissing`, `GhNotAuthenticated`.
  - `token::resolve() -> Result<Token, TokenError>` — lit l'environnement réel et lance `gh auth token`.
  - `token::resolve_from(owl: Option<String>, github: Option<String>, gh: impl FnOnce() -> Result<String, GhFailure>) -> Result<Token, TokenError>` — cœur pur, sans effet de bord.
  - `token::GhFailure` — `enum { NotFound, NotAuthenticated }`, ce que rapporte l'appel à `gh`.

- [ ] **Step 1: Créer la branche de travail**

On est sur `develop`, donc on crée la branche de la fonctionnalité.

```bash
git checkout -b feat/fondations
```

- [ ] **Step 2: Écrire `Cargo.toml`**

Les versions sont épinglées volontairement. `crossterm` est déclaré en direct avec la fonctionnalité `event-stream` (nécessaire pour lire le clavier dans une tâche asynchrone) ; `ratatui 0.30` utilise le même `crossterm 0.29`, cargo unifiera donc en une seule version. `reqwest` a ses fonctionnalités par défaut coupées pour garantir `rustls` et exclure OpenSSL.

```toml
[package]
name = "owl"
version = "0.1.0"
edition = "2021"
description = "Liste et fusionne les pull requests de GitHub depuis le terminal"

[dependencies]
ratatui = "0.30"
crossterm = { version = "0.29", features = ["event-stream"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "process"] }
reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"
thiserror = "2"
anyhow = "1"
directories = "6"
chrono = "0.4"
open = "5"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Écrire `.gitignore`**

```gitignore
/target
```

- [ ] **Step 4: Écrire un `src/main.rs` provisoire**

Juste de quoi compiler. La boucle arrive en tâche 3.

```rust
mod token;

fn main() {
    match token::resolve() {
        Ok(_) => println!("jeton trouvé"),
        Err(erreur) => eprintln!("{erreur}"),
    }
}
```

- [ ] **Step 5: Écrire les tests de `token.rs` en premier**

Créer `src/token.rs` avec **uniquement** ce bloc de tests, pour qu'il échoue.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Fabrique un appel à `gh` factice qui réussit.
    fn gh_ok(sortie: &str) -> impl FnOnce() -> Result<String, GhFailure> + '_ {
        move || Ok(sortie.to_string())
    }

    #[test]
    fn owl_token_gagne_sur_tout_le_reste() {
        let resultat = resolve_from(
            Some("jeton-owl".into()),
            Some("jeton-github".into()),
            gh_ok("jeton-gh"),
        )
        .unwrap();
        assert_eq!(resultat.expose(), "jeton-owl");
    }

    #[test]
    fn github_token_utilise_si_owl_token_absent() {
        let resultat =
            resolve_from(None, Some("jeton-github".into()), gh_ok("jeton-gh")).unwrap();
        assert_eq!(resultat.expose(), "jeton-github");
    }

    #[test]
    fn gh_utilise_en_dernier_recours() {
        let resultat = resolve_from(None, None, gh_ok("jeton-gh")).unwrap();
        assert_eq!(resultat.expose(), "jeton-gh");
    }

    #[test]
    fn une_variable_vide_compte_comme_absente() {
        let resultat = resolve_from(Some("   ".into()), Some("jeton-github".into()), gh_ok("x"))
            .unwrap();
        assert_eq!(resultat.expose(), "jeton-github");
    }

    #[test]
    fn sortie_de_gh_nettoyee_des_espaces() {
        let resultat = resolve_from(None, None, gh_ok("  jeton-gh\n")).unwrap();
        assert_eq!(resultat.expose(), "jeton-gh");
    }

    #[test]
    fn gh_absent_donne_le_message_d_installation() {
        let erreur = resolve_from(None, None, || Err(GhFailure::NotFound)).unwrap_err();
        assert_eq!(
            erreur.to_string(),
            "owl a besoin de gh. Installe-le, puis lance `gh auth login`."
        );
    }

    #[test]
    fn gh_non_connecte_donne_le_message_de_connexion() {
        let erreur = resolve_from(None, None, || Err(GhFailure::NotAuthenticated)).unwrap_err();
        assert_eq!(
            erreur.to_string(),
            "Non connecté à GitHub. Lance `gh auth login`."
        );
    }

    #[test]
    fn gh_qui_renvoie_du_vide_compte_comme_non_connecte() {
        let erreur = resolve_from(None, None, gh_ok("\n")).unwrap_err();
        assert_eq!(
            erreur.to_string(),
            "Non connecté à GitHub. Lance `gh auth login`."
        );
    }

    #[test]
    fn le_jeton_ne_fuit_pas_dans_son_debug() {
        let jeton = resolve_from(Some("ghp_secret".into()), None, gh_ok("x")).unwrap();
        let trace = format!("{jeton:?}");
        assert!(!trace.contains("ghp_secret"), "trace = {trace}");
        assert_eq!(trace, "Token(masqué)");
    }
}
```

- [ ] **Step 6: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --lib token 2>&1 | tail -20`
Expected: échec de compilation, « cannot find function `resolve_from` » et « cannot find type `GhFailure` ».

- [ ] **Step 7: Écrire l'implémentation de `token.rs`**

À insérer **au-dessus** du bloc de tests.

```rust
//! Résolution du jeton d'authentification GitHub.
//!
//! Ordre : `OWL_TOKEN`, `GITHUB_TOKEN`, puis `gh auth token`. Le jeton n'est
//! jamais écrit dans un fichier, ni journalisé, ni affiché.

use std::fmt;
use std::process::Command;

use thiserror::Error;

/// Jeton d'authentification. Son contenu ne sort que par `expose`.
pub struct Token(String);

impl Token {
    /// Donne accès au jeton en clair. Seul l'en-tête HTTP doit s'en servir.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// Masque le contenu : un jeton ne doit jamais apparaître dans une trace.
impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(masqué)")
    }
}

/// Ce que peut rapporter l'appel à `gh auth token`.
#[derive(Debug, PartialEq, Eq)]
pub enum GhFailure {
    /// `gh` n'est pas dans le `PATH`.
    NotFound,
    /// `gh` répond, mais aucune session n'est ouverte.
    NotAuthenticated,
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("owl a besoin de gh. Installe-le, puis lance `gh auth login`.")]
    GhMissing,
    #[error("Non connecté à GitHub. Lance `gh auth login`.")]
    GhNotAuthenticated,
}

/// Résout le jeton depuis l'environnement réel.
pub fn resolve() -> Result<Token, TokenError> {
    resolve_from(
        std::env::var("OWL_TOKEN").ok(),
        std::env::var("GITHUB_TOKEN").ok(),
        run_gh_auth_token,
    )
}

/// Cœur de la résolution, sans effet de bord, donc testable.
pub fn resolve_from(
    owl: Option<String>,
    github: Option<String>,
    gh: impl FnOnce() -> Result<String, GhFailure>,
) -> Result<Token, TokenError> {
    if let Some(valeur) = owl.and_then(non_vide) {
        return Ok(Token(valeur));
    }
    if let Some(valeur) = github.and_then(non_vide) {
        return Ok(Token(valeur));
    }
    match gh() {
        Ok(sortie) => non_vide(sortie)
            .map(Token)
            .ok_or(TokenError::GhNotAuthenticated),
        Err(GhFailure::NotFound) => Err(TokenError::GhMissing),
        Err(GhFailure::NotAuthenticated) => Err(TokenError::GhNotAuthenticated),
    }
}

/// Rend `None` pour une chaîne vide ou faite d'espaces.
fn non_vide(valeur: String) -> Option<String> {
    let taille = valeur.trim();
    if taille.is_empty() {
        None
    } else {
        Some(taille.to_string())
    }
}

/// Lance `gh auth token`. Un `gh` introuvable et un `gh` déconnecté sont deux
/// situations différentes, avec deux messages différents.
fn run_gh_auth_token() -> Result<String, GhFailure> {
    let sortie = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|_| GhFailure::NotFound)?;

    if !sortie.status.success() {
        return Err(GhFailure::NotAuthenticated);
    }

    String::from_utf8(sortie.stdout).map_err(|_| GhFailure::NotAuthenticated)
}
```

- [ ] **Step 8: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test --lib token`
Expected: 9 tests réussis.

- [ ] **Step 9: Vérifications obligatoires**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe. `cargo build` produit `target/debug/owl`.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore src/main.rs src/token.rs
git commit -m "Ajoute le squelette du projet et la résolution du jeton"
```

---

### Task 2: Lecture et validation des réglages

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: rien de la tâche 1.
- Produces:
  - `config::Config` — `struct` publique : `filters: Vec<String>`, `refresh_interval: u64`, `preferred_merge_method: MergeMethod`, `page_size: u16`. Dérive `Debug`, `Clone`, `PartialEq`.
  - `config::Config::default()` — les valeurs par défaut de la spec.
  - `config::MergeMethod` — `enum { Squash, Rebase, Merge }`, dérive `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`.
  - `config::default_path() -> Result<PathBuf, ConfigError>` — `~/.config/owl/config.toml`.
  - `config::load() -> Result<Config, ConfigError>` — chemin par défaut, fichier absent = valeurs par défaut.
  - `config::load_from(path: &Path) -> Result<Config, ConfigError>` — cœur testable.
  - `config::ConfigError` — `enum` `thiserror` : `Syntax { path: String }`, `InvalidKey { path: String, key: String }`, `EmptyFilters`, `NoHomeDirectory`.

- [ ] **Step 1: Écrire les tests de `config.rs` en premier**

Créer `src/config.rs` avec **uniquement** ce bloc de tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Écrit un fichier de réglages temporaire et le lit.
    fn lire(contenu: &str) -> Result<Config, ConfigError> {
        let mut fichier = NamedTempFile::new().unwrap();
        fichier.write_all(contenu.as_bytes()).unwrap();
        fichier.flush().unwrap();
        load_from(fichier.path())
    }

    #[test]
    fn fichier_absent_donne_les_valeurs_par_defaut() {
        let reglages = load_from(Path::new("/introuvable/owl/config.toml")).unwrap();
        assert_eq!(reglages, Config::default());
    }

    #[test]
    fn valeurs_par_defaut_conformes_a_la_spec() {
        let reglages = Config::default();
        assert_eq!(reglages.filters, vec!["author:@me", "is:open"]);
        assert_eq!(reglages.refresh_interval, 60);
        assert_eq!(reglages.preferred_merge_method, MergeMethod::Squash);
        assert_eq!(reglages.page_size, 50);
    }

    #[test]
    fn fichier_vide_donne_les_valeurs_par_defaut() {
        assert_eq!(lire("").unwrap(), Config::default());
    }

    #[test]
    fn fichier_complet_lu_entierement() {
        let reglages = lire(
            r#"
filters = ["review-requested:@me"]
refresh_interval = 0
preferred_merge_method = "rebase"
page_size = 100
"#,
        )
        .unwrap();
        assert_eq!(reglages.filters, vec!["review-requested:@me"]);
        assert_eq!(reglages.refresh_interval, 0);
        assert_eq!(reglages.preferred_merge_method, MergeMethod::Rebase);
        assert_eq!(reglages.page_size, 100);
    }

    #[test]
    fn cle_inconnue_ignoree_sans_erreur() {
        let reglages = lire("couleur_preferee = \"bleu\"\nrefresh_interval = 30\n").unwrap();
        assert_eq!(reglages.refresh_interval, 30);
    }

    #[test]
    fn les_trois_methodes_de_fusion_sont_acceptees() {
        for (texte, attendu) in [
            ("squash", MergeMethod::Squash),
            ("rebase", MergeMethod::Rebase),
            ("merge", MergeMethod::Merge),
        ] {
            let reglages = lire(&format!("preferred_merge_method = \"{texte}\"\n")).unwrap();
            assert_eq!(reglages.preferred_merge_method, attendu);
        }
    }

    #[test]
    fn methode_de_fusion_inconnue_refusee_avec_sa_cle() {
        let erreur = lire("preferred_merge_method = \"fast-forward\"\n").unwrap_err();
        let message = erreur.to_string();
        assert!(message.starts_with("Réglages invalides dans "), "{message}");
        assert!(message.ends_with(" : preferred_merge_method."), "{message}");
    }

    #[test]
    fn page_size_hors_bornes_refusee_avec_sa_cle() {
        for valeur in ["0", "101", "-5"] {
            let erreur = lire(&format!("page_size = {valeur}\n")).unwrap_err();
            assert!(
                erreur.to_string().ends_with(" : page_size."),
                "valeur {valeur} → {erreur}"
            );
        }
    }

    #[test]
    fn mauvais_type_refuse_avec_sa_cle() {
        let erreur = lire("page_size = \"beaucoup\"\n").unwrap_err();
        assert!(erreur.to_string().ends_with(" : page_size."), "{erreur}");

        let erreur = lire("filters = \"author:@me\"\n").unwrap_err();
        assert!(erreur.to_string().ends_with(" : filters."), "{erreur}");

        let erreur = lire("refresh_interval = -1\n").unwrap_err();
        assert!(
            erreur.to_string().ends_with(" : refresh_interval."),
            "{erreur}"
        );
    }

    #[test]
    fn liste_de_filtres_vide_refusee_avec_son_propre_message() {
        let erreur = lire("filters = []\n").unwrap_err();
        assert_eq!(
            erreur.to_string(),
            "Aucun filtre actif : la recherche ramènerait tout GitHub."
        );
    }

    #[test]
    fn syntaxe_toml_invalide_refusee_avec_le_chemin() {
        let erreur = lire("filters = [\n").unwrap_err();
        let message = erreur.to_string();
        assert!(message.starts_with("Réglages invalides dans "), "{message}");
        assert!(message.ends_with(" : syntaxe TOML invalide."), "{message}");
    }

    #[test]
    fn le_chemin_par_defaut_est_dans_config_owl() {
        let chemin = default_path().unwrap();
        assert!(
            chemin.ends_with("owl/config.toml"),
            "chemin = {}",
            chemin.display()
        );
    }
}
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --lib config 2>&1 | tail -20`
Expected: échec de compilation, « cannot find type `Config` ».

- [ ] **Step 3: Écrire l'implémentation de `config.rs`**

On lit le fichier comme une table TOML brute, puis on valide clé par clé. C'est ce qui permet de nommer la clé fautive dans le message : une désérialisation directe vers `Config` ne donnerait qu'un message générique.

```rust
//! Lecture du fichier de réglages `~/.config/owl/config.toml`.
//!
//! Le fichier est optionnel. Une clé inconnue est ignorée. Une valeur invalide
//! arrête le programme avec un message qui nomme la clé fautive.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Méthode de fusion présélectionnée quand le dépôt en autorise plusieurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Squash,
    Rebase,
    Merge,
}

/// Réglages effectifs du programme.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Filtres actifs au démarrage, dans la syntaxe de recherche de GitHub.
    pub filters: Vec<String>,
    /// Intervalle de rafraîchissement en secondes. 0 désactive le minuteur.
    pub refresh_interval: u64,
    /// Méthode de fusion préférée. Utilisée par `04-fusion.md`.
    #[allow(dead_code)] // consommé par la spec 04, pas encore lu ici
    pub preferred_merge_method: MergeMethod,
    /// Nombre maximal de PR ramenées par requête, de 1 à 100.
    pub page_size: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            filters: vec!["author:@me".to_string(), "is:open".to_string()],
            refresh_interval: 60,
            preferred_merge_method: MergeMethod::Squash,
            page_size: 50,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Réglages invalides dans {path} : syntaxe TOML invalide.")]
    Syntax { path: String },
    #[error("Réglages invalides dans {path} : {key}.")]
    InvalidKey { path: String, key: String },
    #[error("Aucun filtre actif : la recherche ramènerait tout GitHub.")]
    EmptyFilters,
    #[error("Impossible de déterminer le dossier de configuration.")]
    NoHomeDirectory,
}

/// Chemin du fichier de réglages : `~/.config/owl/config.toml`.
///
/// On construit le chemin depuis le dossier personnel, et non avec
/// `ProjectDirs`, qui donnerait `~/Library/Application Support/owl` sur macOS
/// alors que la spec impose `~/.config/owl` partout.
pub fn default_path() -> Result<PathBuf, ConfigError> {
    let base = directories::BaseDirs::new().ok_or(ConfigError::NoHomeDirectory)?;
    Ok(base.home_dir().join(".config").join("owl").join("config.toml"))
}

/// Lit les réglages au chemin par défaut.
pub fn load() -> Result<Config, ConfigError> {
    load_from(&default_path()?)
}

/// Lit les réglages à un chemin donné. Fichier absent ou illisible en lecture :
/// valeurs par défaut, parce que la spec rend le fichier optionnel.
pub fn load_from(path: &Path) -> Result<Config, ConfigError> {
    let Ok(texte) = std::fs::read_to_string(path) else {
        return Ok(Config::default());
    };

    let affichage = path.display().to_string();
    let table: toml::Table = toml::from_str(&texte).map_err(|_| ConfigError::Syntax {
        path: affichage.clone(),
    })?;

    let mut reglages = Config::default();

    let invalide = |cle: &str| ConfigError::InvalidKey {
        path: affichage.clone(),
        key: cle.to_string(),
    };

    if let Some(valeur) = table.get("filters") {
        let liste = valeur.as_array().ok_or_else(|| invalide("filters"))?;
        let mut filtres = Vec::with_capacity(liste.len());
        for element in liste {
            let texte = element.as_str().ok_or_else(|| invalide("filters"))?;
            filtres.push(texte.to_string());
        }
        if filtres.is_empty() {
            return Err(ConfigError::EmptyFilters);
        }
        reglages.filters = filtres;
    }

    if let Some(valeur) = table.get("refresh_interval") {
        let secondes = valeur
            .as_integer()
            .ok_or_else(|| invalide("refresh_interval"))?;
        reglages.refresh_interval = u64::try_from(secondes)
            .map_err(|_| invalide("refresh_interval"))?;
    }

    if let Some(valeur) = table.get("preferred_merge_method") {
        let texte = valeur
            .as_str()
            .ok_or_else(|| invalide("preferred_merge_method"))?;
        reglages.preferred_merge_method = match texte {
            "squash" => MergeMethod::Squash,
            "rebase" => MergeMethod::Rebase,
            "merge" => MergeMethod::Merge,
            _ => return Err(invalide("preferred_merge_method")),
        };
    }

    if let Some(valeur) = table.get("page_size") {
        let nombre = valeur.as_integer().ok_or_else(|| invalide("page_size"))?;
        if !(1..=100).contains(&nombre) {
            return Err(invalide("page_size"));
        }
        reglages.page_size = nombre as u16;
    }

    Ok(reglages)
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test --lib config`
Expected: 12 tests réussis.

Si `toml::Table` n'existe pas sous ce nom, utiliser `toml::value::Table` — c'est le même type réexporté.

- [ ] **Step 5: Brancher `config` dans `main.rs`**

Remplacer le contenu de `src/main.rs` par :

```rust
mod config;
mod token;

fn main() {
    match (config::load(), token::resolve()) {
        (Ok(reglages), Ok(_)) => println!("{} filtres actifs", reglages.filters.len()),
        (Err(erreur), _) => eprintln!("{erreur}"),
        (_, Err(erreur)) => eprintln!("{erreur}"),
    }
}
```

- [ ] **Step 6: Vérifications obligatoires**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe.

- [ ] **Step 7: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "Ajoute la lecture et la validation du fichier de réglages"
```

---

### Task 3: Écran, garde du terminal et messages de démarrage

**Files:**
- Modify: `src/main.rs` (remplacement complet)
- Create: `tests/startup.rs`

**Interfaces:**
- Consumes: `config::load()`, `config::Config`, `token::resolve()`, `token::Token`.
- Produces:
  - `main::TerminalGuard` — garde de portée privée : `enter() -> anyhow::Result<(Terminal<CrosstermBackend<Stdout>>, TerminalGuard)>`, et un `Drop` qui restaure le terminal.
  - Le binaire : code de sortie 0 en cas de sortie normale, non nul après un message d'erreur de démarrage.

À ce stade l'écran affiche un cadre fixe et « q » quitte. La boucle d'événements complète arrive en tâche 5.

- [ ] **Step 1: Écrire les tests d'intégration en premier**

Créer `tests/startup.rs`. Ces tests lancent le vrai binaire dans un environnement dépouillé : aucune variable de jeton, et un `PATH` vide pour que `gh` soit introuvable.

```rust
//! Tests des erreurs de démarrage, sur le binaire réel.

use std::process::Command;

/// Lance `owl` sans aucune variable d'environnement et sans `gh` accessible.
fn owl_sans_authentification() -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_owl"))
        .env_clear()
        .env("PATH", "")
        .env("HOME", "/introuvable-owl")
        .output()
        .expect("le binaire owl doit être exécutable")
}

#[test]
fn sans_jeton_ni_gh_le_message_indique_d_installer_gh() {
    let sortie = owl_sans_authentification();
    let erreur = String::from_utf8_lossy(&sortie.stderr);
    assert_eq!(
        erreur.trim(),
        "owl a besoin de gh. Installe-le, puis lance `gh auth login`."
    );
}

#[test]
fn sans_jeton_le_code_de_sortie_est_non_nul() {
    let sortie = owl_sans_authentification();
    assert!(
        !sortie.status.success(),
        "code de sortie = {:?}",
        sortie.status.code()
    );
}

#[test]
fn sans_jeton_rien_n_est_ecrit_sur_la_sortie_standard() {
    let sortie = owl_sans_authentification();
    let standard = String::from_utf8_lossy(&sortie.stdout);
    assert!(
        standard.is_empty(),
        "la sortie standard doit rester vide, pas de séquence d'échappement : {standard:?}"
    );
    assert!(
        !standard.contains('\u{1b}'),
        "aucune séquence d'échappement ne doit salir le terminal"
    );
}
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --test startup 2>&1 | tail -20`
Expected: échec — le binaire actuel n'écrit pas ce message et rend un code 0.

- [ ] **Step 3: Écrire le nouveau `src/main.rs`**

Remplacement complet du fichier.

```rust
//! Démarrage de `owl` : réglages, jeton, écran, restauration du terminal.

mod config;
mod token;

use std::io::{self, Stdout};
use std::process::ExitCode;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

fn main() -> ExitCode {
    // Les erreurs de démarrage sont écrites avant toute prise de contrôle du
    // terminal, donc jamais avalées par l'écran alterné.
    let reglages = match config::load() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    let jeton = match token::resolve() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    match run(reglages, jeton) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erreur) => {
            eprintln!("{erreur}");
            ExitCode::FAILURE
        }
    }
}

/// Restaure le terminal à la sortie de portée, quelle qu'en soit la cause.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

type Ecran = Terminal<CrosstermBackend<Stdout>>;

/// Prend le contrôle du terminal et installe le crochet de panique.
/// Le garde et le crochet font le même travail : le garde couvre la sortie
/// normale et l'erreur, le crochet couvre la panique.
fn enter_terminal() -> Result<(Ecran, TerminalGuard)> {
    let crochet_precedent = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |infos| {
        let _ = restore_terminal();
        crochet_precedent(infos);
    }));

    enable_raw_mode()?;
    let mut sortie = io::stdout();
    execute!(sortie, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(sortie))?;
    Ok((terminal, TerminalGuard))
}

/// Rend le terminal à l'utilisateur. Volontairement tolérante aux erreurs :
/// elle est appelée depuis un `Drop` et depuis un crochet de panique.
fn restore_terminal() -> Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    Ok(())
}

fn run(reglages: config::Config, _jeton: token::Token) -> Result<()> {
    let (mut terminal, _garde) = enter_terminal()?;

    loop {
        terminal.draw(|cadre| {
            let contenu = Paragraph::new(format!(
                "{} filtres actifs — « q » pour quitter",
                reglages.filters.len()
            ))
            .block(Block::default().borders(Borders::ALL).title(" owl "));
            cadre.render_widget(contenu, cadre.area());
        })?;

        if let Event::Key(touche) = event::read()? {
            if touche.kind == KeyEventKind::Press && touche.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Lancer les tests d'intégration pour vérifier qu'ils passent**

Run: `cargo test --test startup`
Expected: 3 tests réussis.

- [ ] **Step 5: Vérifier l'ouverture et la fermeture de l'écran**

`owl` exige un vrai terminal : sans lui, `enable_raw_mode` échoue. La commande `script` de macOS en fabrique un et capture tout ce qui s'y affiche, séquences d'échappement comprises. C'est ce qui rend cette vérification automatisable, sans personne devant le clavier.

Run:

```bash
cargo build --quiet
printf 'q' | script -q /dev/null ./target/debug/owl > /tmp/owl-ecran.txt 2>&1
echo "code de sortie : $?"
grep -c 'owl — pull requests' /tmp/owl-ecran.txt
grep -c '1049h' /tmp/owl-ecran.txt
grep -c '1049l' /tmp/owl-ecran.txt
```

Attendu : code de sortie 0, et les trois `grep` rendent au moins 1. Ils prouvent respectivement que le cadre a été dessiné, que l'écran alterné a été pris (`1049h`), et qu'il a été rendu (`1049l`) — donc que « q » a bien refermé proprement.

Si `owl` ne trouve pas de jeton dans cet environnement, la commande s'arrête sur le message d'erreur de démarrage : exporter un jeton d'abord avec `export GITHUB_TOKEN=$(gh auth token)` pour la durée de la vérification.

- [ ] **Step 6: Vérifier qu'une panique rend un terminal utilisable**

Ajouter temporairement, juste après `let (mut terminal, _garde) = enter_terminal()?;` :

```rust
panic!("test du crochet de panique");
```

Run:

```bash
cargo build --quiet
script -q /dev/null ./target/debug/owl > /tmp/owl-panique.txt 2>&1
grep -c 'test du crochet de panique' /tmp/owl-panique.txt
grep -c '1049l' /tmp/owl-panique.txt
```

Attendu : les deux `grep` rendent au moins 1. La trace de panique est lisible, et la présence de `1049l` prouve que le crochet a rendu le terminal avant de laisser la panique remonter.

Puis **retirer la ligne `panic!`**, relancer `cargo build` et confirmer par `grep -c 'panic!' src/main.rs` qui doit rendre 0.

- [ ] **Step 7: Vérifications obligatoires**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs tests/startup.rs
git commit -m "Ouvre et restaure l'écran du terminal, avec garde et crochet de panique"
```

---

### Task 4: État de l'application, générations et modules du domaine

**Files:**
- Create: `src/model.rs`
- Create: `src/filter.rs`
- Create: `src/github/mod.rs`
- Create: `src/github/queries.rs`
- Create: `src/github/dto.rs`
- Create: `src/app.rs`
- Modify: `src/main.rs` (déclaration des modules seulement)

**Interfaces:**
- Consumes: `config::Config`, `config::MergeMethod`.
- Produces:
  - `model::PullRequest` — `struct` publique : `repository: String`, `number: u32`, `title: String`. Dérive `Debug`, `Clone`, `PartialEq`.
  - `app::Generation` — alias `pub type Generation = u64;`
  - `app::Key` — `enum { Char(char), Other }`. `app` ne dépend pas de `crossterm` : c'est `main` qui traduit.
  - `app::Event` — `enum { Key(Key), Tick, Data { generation: Generation, result: Result<Vec<PullRequest>, String> } }`
  - `app::Command` — `enum { Fetch { generation: Generation, filters: Vec<String>, page_size: u16 }, Quit }`. Dérive `Debug`, `PartialEq`.
  - `app::App` — champs publics en lecture : `items: Vec<PullRequest>`, `status: String`, `loading: bool`, `should_quit: bool`, `last_refresh: Option<DateTime<Local>>`.
  - `app::App::new(config: Config) -> App`
  - `app::App::start(&mut self) -> Vec<Command>` — la première requête.
  - `app::App::handle(&mut self, event: Event) -> Vec<Command>`
  - `github::fetch_pull_requests(token: &str, filters: &[String], page_size: u16) -> Result<Vec<PullRequest>, String>` — bouchon au stade fondations, remplacé par la spec 01.

- [ ] **Step 1: Écrire `src/model.rs`**

```rust
//! Types métier. Ne dépend ni du réseau ni du terminal.

/// Une pull request telle qu'affichée dans la liste.
/// La spec `01-modele-et-donnees.md` étend ce type.
#[derive(Debug, Clone, PartialEq)]
pub struct PullRequest {
    /// Dépôt au format `proprietaire/nom`.
    pub repository: String,
    pub number: u32,
    pub title: String,
}
```

- [ ] **Step 2: Écrire les modules encore vides**

`src/filter.rs` :

```rust
//! Filtres et construction de la requête de recherche GitHub.
//!
//! Vide au stade des fondations : le type `Filter` et `build_query` sont
//! définis par `docs/specs/02-filtres.md`. Jusque-là, les filtres circulent
//! sous forme de chaînes, telles qu'écrites dans le fichier de réglages.
```

`src/github/queries.rs` :

```rust
//! Requêtes GraphQL et mutation de fusion.
//!
//! Vide au stade des fondations. Contenu défini par
//! `docs/specs/01-modele-et-donnees.md`.
```

`src/github/dto.rs` :

```rust
//! Types de réponse brute de l'API, mappés vers `model`.
//!
//! Vide au stade des fondations. Contenu défini par
//! `docs/specs/01-modele-et-donnees.md`.
```

`src/github/mod.rs` :

```rust
//! Client GraphQL de GitHub.

pub mod dto;
pub mod queries;

use crate::model::PullRequest;

/// Ramène les pull requests correspondant aux filtres.
///
/// Bouchon au stade des fondations : renvoie une liste vide sans toucher au
/// réseau, ce qui suffit à faire tourner la boucle d'événements et le
/// mécanisme de générations. Le vrai client est défini par
/// `docs/specs/01-modele-et-donnees.md`, qui remplacera aussi le `String`
/// d'erreur par un type `thiserror`.
pub async fn fetch_pull_requests(
    _token: &str,
    _filters: &[String],
    _page_size: u16,
) -> Result<Vec<PullRequest>, String> {
    Ok(Vec::new())
}
```

- [ ] **Step 3: Écrire les tests de `app.rs` en premier**

Créer `src/app.rs` avec **uniquement** ce bloc de tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn pr(numero: u32) -> PullRequest {
        PullRequest {
            repository: "moi/depot".to_string(),
            number: numero,
            title: format!("Titre {numero}"),
        }
    }

    /// Application démarrée, première requête émise, génération courante rendue.
    fn app_demarree() -> (App, Generation) {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        let generation = match &commandes[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        (app, generation)
    }

    #[test]
    fn le_demarrage_emet_une_seule_requete() {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        assert_eq!(
            commandes,
            vec![Command::Fetch {
                generation: 1,
                filters: vec!["author:@me".to_string(), "is:open".to_string()],
                page_size: 50,
            }]
        );
        assert!(app.loading);
    }

    #[test]
    fn q_demande_la_sortie() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn r_relance_une_requete_avec_une_generation_plus_recente() {
        let (mut app, premiere) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Char('r')));
        match &commandes[0] {
            Command::Fetch { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert!(app.loading);
    }

    #[test]
    fn le_minuteur_relance_une_requete() {
        let (mut app, premiere) = app_demarree();
        let commandes = app.handle(Event::Tick);
        match &commandes[0] {
            Command::Fetch { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn une_touche_inconnue_ne_fait_rien() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Other));
        assert!(commandes.is_empty());
        assert!(!app.should_quit);
    }

    #[test]
    fn un_resultat_a_jour_remplace_la_liste() {
        let (mut app, generation) = app_demarree();
        let commandes = app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(1), pr(2)]),
        });
        assert!(commandes.is_empty());
        assert_eq!(app.items, vec![pr(1), pr(2)]);
        assert!(!app.loading);
        assert!(app.last_refresh.is_some());
    }

    #[test]
    fn un_resultat_perime_est_ignore() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(1)]),
        });
        // Une nouvelle requête part, puis la réponse lente de l'ancienne arrive.
        app.handle(Event::Key(Key::Char('r')));
        let commandes = app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(99)]),
        });
        assert!(commandes.is_empty());
        assert_eq!(app.items, vec![pr(1)], "la réponse lente ne doit rien écraser");
        assert!(app.loading, "la requête en cours reste en cours");
    }

    #[test]
    fn une_erreur_laisse_la_liste_affichee() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(1)]),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Err("Réseau injoignable".to_string()),
        });
        assert_eq!(app.items, vec![pr(1)], "la liste précédente reste visible");
        assert_eq!(app.status, "Réseau injoignable", "message repris tel quel");
        assert!(!app.loading);
    }

    #[test]
    fn un_succes_efface_l_erreur_en_cours() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Err("Réseau injoignable".to_string()),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(1)]),
        });
        assert!(
            !app.status.contains("Réseau injoignable"),
            "status = {}",
            app.status
        );
    }

    #[test]
    fn le_status_annonce_le_nombre_de_pull_requests() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(1), pr(2)]),
        });
        assert!(app.status.starts_with("2 pull requests"), "{}", app.status);

        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Ok(vec![]),
        });
        assert_eq!(app.status, "Aucune pull request");
    }

    #[test]
    fn les_filtres_des_reglages_sont_transmis_a_la_requete() {
        let reglages = Config {
            filters: vec!["review-requested:@me".to_string()],
            page_size: 7,
            ..Config::default()
        };
        let mut app = App::new(reglages);
        assert_eq!(
            app.start(),
            vec![Command::Fetch {
                generation: 1,
                filters: vec!["review-requested:@me".to_string()],
                page_size: 7,
            }]
        );
    }
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --lib app 2>&1 | tail -20`
Expected: échec de compilation, « cannot find type `App` ».

- [ ] **Step 5: Écrire l'implémentation de `app.rs`**

À insérer **au-dessus** du bloc de tests.

```rust
//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

use chrono::{DateTime, Local};

use crate::config::Config;
use crate::model::PullRequest;

/// Numéro de génération d'une demande réseau. Un résultat dont la génération
/// est périmée est ignoré, ce qui évite qu'une réponse lente écrase une
/// réponse plus récente.
pub type Generation = u64;

/// Touche reçue, traduite par `main`. `app` ignore volontairement `crossterm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    /// Toute autre touche, sans effet au stade des fondations.
    Other,
}

/// Ce qui arrive dans la file d'événements.
#[derive(Debug)]
pub enum Event {
    Key(Key),
    /// Tour de minuteur de rafraîchissement.
    Tick,
    /// Résultat d'une demande réseau.
    Data {
        generation: Generation,
        result: Result<Vec<PullRequest>, String>,
    },
}

/// Ce que `app` demande à `main` de faire.
#[derive(Debug, PartialEq)]
pub enum Command {
    Fetch {
        generation: Generation,
        filters: Vec<String>,
        page_size: u16,
    },
    Quit,
}

pub struct App {
    pub items: Vec<PullRequest>,
    /// Ligne affichée dans la barre d'état, prête à dessiner.
    pub status: String,
    pub loading: bool,
    pub should_quit: bool,
    pub last_refresh: Option<DateTime<Local>>,
    generation: Generation,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            items: Vec::new(),
            status: "Chargement…".to_string(),
            loading: false,
            should_quit: false,
            last_refresh: None,
            generation: 0,
            config,
        }
    }

    /// Première demande, au démarrage.
    pub fn start(&mut self) -> Vec<Command> {
        vec![self.fetch()]
    }

    pub fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Key(Key::Char('q')) => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            Event::Key(Key::Char('r')) | Event::Tick => vec![self.fetch()],
            Event::Key(_) => Vec::new(),
            Event::Data { generation, result } => {
                // Réponse d'une demande dépassée : on la jette sans rien changer.
                if generation != self.generation {
                    return Vec::new();
                }
                self.loading = false;
                match result {
                    Ok(items) => {
                        self.items = items;
                        self.last_refresh = Some(Local::now());
                        self.status = self.liste_resumee();
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(message) => self.status = message,
                }
                Vec::new()
            }
        }
    }

    /// Ouvre une nouvelle génération et demande les données.
    fn fetch(&mut self) -> Command {
        self.generation += 1;
        self.loading = true;
        Command::Fetch {
            generation: self.generation,
            filters: self.config.filters.clone(),
            page_size: self.config.page_size,
        }
    }

    /// Résumé de la liste pour la barre d'état.
    fn liste_resumee(&self) -> String {
        match self.items.len() {
            0 => "Aucune pull request".to_string(),
            1 => "1 pull request".to_string(),
            nombre => format!("{nombre} pull requests"),
        }
    }
}
```

- [ ] **Step 6: Déclarer les nouveaux modules dans `main.rs`**

Remplacer le bloc de déclarations en tête de `src/main.rs` par :

```rust
mod app;
mod config;
mod filter;
mod github;
mod model;
mod token;
```

- [ ] **Step 7: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test`
Expected: tous les tests réussis, dont les 11 de `app`.

- [ ] **Step 8: Vérifications obligatoires**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe. Si clippy signale un champ jamais lu de `App` (par exemple `last_refresh` avant la tâche 5), ne pas ajouter d'attribut : la tâche 5 le lit dans `ui`, il suffit d'enchaîner.

- [ ] **Step 9: Commit**

```bash
git add src/app.rs src/model.rs src/filter.rs src/github src/main.rs
git commit -m "Ajoute l'état de l'application, les générations et les modules du domaine"
```

---

### Task 5: Boucle d'événements à trois producteurs, dessin et notes de spec

**Files:**
- Modify: `src/main.rs` (remplacement de `run`, ajout de la boucle)
- Create: `src/ui/mod.rs`
- Create: `src/ui/list.rs`
- Create: `src/ui/detail.rs`
- Create: `src/ui/merge.rs`
- Modify: `docs/specs/01-modele-et-donnees.md`
- Modify: `docs/specs/02-filtres.md`
- Modify: `docs/specs/03-affichage-et-navigation.md`
- Modify: `docs/specs/04-fusion.md`

**Interfaces:**
- Consumes: `app::App`, `app::Event`, `app::Key`, `app::Command`, `github::fetch_pull_requests`, `token::Token`, `config::Config`.
- Produces:
  - `ui::draw(frame: &mut Frame, app: &App)` — aiguillage de dessin. Prend `app` en lecture seule.
  - `ui::list::draw(frame: &mut Frame, area: Rect, app: &App)`

- [ ] **Step 1: Écrire `src/ui/list.rs`**

Aucune décision ici : les textes viennent de `app`.

```rust
//! Dessin de la liste des pull requests et de la barre d'état.
//!
//! Aucune décision : les messages et les résumés sont préparés par `app`.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let lignes: Vec<ListItem> = app
        .items
        .iter()
        .map(|pr| {
            ListItem::new(format!(
                "{}#{}  {}",
                pr.repository, pr.number, pr.title
            ))
        })
        .collect();

    let liste = List::new(lignes).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" owl — pull requests "),
    );
    frame.render_widget(liste, zones[0]);

    let heure = app
        .last_refresh
        .map(|instant| format!(" · mis à jour à {}", instant.format("%H:%M")))
        .unwrap_or_default();
    let attente = if app.loading { " · chargement…" } else { "" };
    let barre = Paragraph::new(format!(
        "{}{heure}{attente} · q quitter · r rafraîchir",
        app.status
    ));
    frame.render_widget(barre, zones[1]);
}
```

- [ ] **Step 2: Écrire `src/ui/mod.rs` et les deux modules encore vides**

`src/ui/mod.rs` :

```rust
//! Aiguillage de dessin. Lit `app` en lecture seule et ne décide de rien.

pub mod detail;
pub mod list;
pub mod merge;

use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    // Une seule vue au stade des fondations. L'aiguillage selon la vue
    // courante est défini par `docs/specs/03-affichage-et-navigation.md`.
    list::draw(frame, frame.area(), app);
}
```

`src/ui/detail.rs` :

```rust
//! Dessin de la vue détail d'une pull request.
//!
//! Vide au stade des fondations. Contenu défini par
//! `docs/specs/03-affichage-et-navigation.md`.
```

`src/ui/merge.rs` :

```rust
//! Dessin de la fenêtre de confirmation de fusion.
//!
//! Vide au stade des fondations. Contenu défini par `docs/specs/04-fusion.md`.
```

- [ ] **Step 3: Remplacer `run` dans `src/main.rs` par la boucle à trois producteurs**

Le fichier est réécrit en entier. Deux choix à comprendre avant de le copier :

- **Le clavier est lu dans une tâche bloquante**, avec `crossterm::event::poll` puis `read`, et non avec `EventStream`. `EventStream` demanderait `tokio_stream` ou `futures`, qui ne sont pas dans les bibliothèques imposées ; une tâche bloquante dédiée satisfait la spec (« les touches du clavier, lues dans une tâche dédiée ») sans rien ajouter.
- **L'exécuteur `tokio` est construit à la main**, sans `#[tokio::main]`, parce qu'il doit naître *après* la lecture des réglages et du jeton : une erreur de démarrage ne doit rien coûter.

Le fichier final `src/main.rs` :

```rust
//! Démarrage de `owl` : réglages, jeton, écran, boucle d'événements.

mod app;
mod config;
mod filter;
mod github;
mod model;
mod token;
mod ui;

use std::io::{self, Stdout};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event as TerminalEvent, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::app::{App, Command, Event, Key};

fn main() -> ExitCode {
    // Les erreurs de démarrage sont écrites avant toute prise de contrôle du
    // terminal, donc jamais avalées par l'écran alterné.
    let reglages = match config::load() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    let jeton = match token::resolve() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    // L'exécuteur n'est construit qu'après les vérifications de démarrage.
    let execution = match tokio::runtime::Runtime::new() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    match execution.block_on(run(reglages, jeton)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erreur) => {
            eprintln!("{erreur}");
            ExitCode::FAILURE
        }
    }
}

/// Restaure le terminal à la sortie de portée, quelle qu'en soit la cause.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

type Ecran = Terminal<CrosstermBackend<Stdout>>;

/// Prend le contrôle du terminal et installe le crochet de panique.
/// Le garde couvre la sortie normale et l'erreur, le crochet couvre la panique.
fn enter_terminal() -> Result<(Ecran, TerminalGuard)> {
    let crochet_precedent = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |infos| {
        let _ = restore_terminal();
        crochet_precedent(infos);
    }));

    enable_raw_mode()?;
    let mut sortie = io::stdout();
    execute!(sortie, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(sortie))?;
    Ok((terminal, TerminalGuard))
}

/// Rend le terminal à l'utilisateur. Volontairement tolérante aux erreurs :
/// elle est appelée depuis un `Drop` et depuis un crochet de panique.
fn restore_terminal() -> Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    Ok(())
}

async fn run(reglages: config::Config, jeton: token::Token) -> Result<()> {
    let intervalle = reglages.refresh_interval;
    let jeton = Arc::new(jeton);
    let mut etat = App::new(reglages);

    let (envoi, mut reception) = mpsc::unbounded_channel::<Event>();

    // Producteur 1 : le clavier, dans une tâche bloquante dédiée.
    spawn_keyboard(envoi.clone());

    // Producteur 2 : le minuteur de rafraîchissement, si activé.
    if intervalle > 0 {
        spawn_timer(envoi.clone(), intervalle);
    }

    let (mut terminal, _garde) = enter_terminal()?;

    // Producteur 3 : les résultats réseau, une tâche par demande.
    for commande in etat.start() {
        execute_command(commande, &envoi, &jeton);
    }
    terminal.draw(|cadre| ui::draw(cadre, &etat))?;

    while let Some(evenement) = reception.recv().await {
        for commande in etat.handle(evenement) {
            execute_command(commande, &envoi, &jeton);
        }
        if etat.should_quit {
            break;
        }
        terminal.draw(|cadre| ui::draw(cadre, &etat))?;
    }

    Ok(())
}

/// Exécute une commande émise par `app`. C'est le seul endroit où le jeton
/// circule : il n'entre jamais dans `app` ni dans `ui`.
fn execute_command(commande: Command, envoi: &UnboundedSender<Event>, jeton: &Arc<token::Token>) {
    match commande {
        Command::Quit => {}
        Command::Fetch {
            generation,
            filters,
            page_size,
        } => {
            let envoi = envoi.clone();
            let jeton = jeton.clone();
            tokio::spawn(async move {
                let resultat =
                    github::fetch_pull_requests(jeton.expose(), &filters, page_size).await;
                let _ = envoi.send(Event::Data {
                    generation,
                    result: resultat,
                });
            });
        }
    }
}

/// Lit le clavier dans une tâche bloquante et traduit les touches pour `app`.
/// La traduction est faite ici pour que `app` ne dépende pas de `crossterm`.
fn spawn_keyboard(envoi: UnboundedSender<Event>) {
    tokio::task::spawn_blocking(move || loop {
        // Le sondage évite de bloquer indéfiniment sur un canal fermé.
        match crossterm::event::poll(Duration::from_millis(200)) {
            Ok(true) => {}
            Ok(false) => {
                if envoi.is_closed() {
                    return;
                }
                continue;
            }
            Err(_) => return,
        }

        let Ok(TerminalEvent::Key(touche)) = crossterm::event::read() else {
            continue;
        };
        if touche.kind != KeyEventKind::Press {
            continue;
        }
        let traduite = match touche.code {
            KeyCode::Char(caractere) => Key::Char(caractere),
            _ => Key::Other,
        };
        if envoi.send(Event::Key(traduite)).is_err() {
            return;
        }
    });
}

/// Émet un `Tick` à intervalle régulier.
fn spawn_timer(envoi: UnboundedSender<Event>, secondes: u64) {
    tokio::spawn(async move {
        let mut minuteur = tokio::time::interval(Duration::from_secs(secondes));
        // Le premier tour part immédiatement : on le consomme, `start` a déjà
        // lancé la requête initiale.
        minuteur.tick().await;
        loop {
            minuteur.tick().await;
            if envoi.send(Event::Tick).is_err() {
                return;
            }
        }
    });
}
```

- [ ] **Step 4: Lancer les tests**

Run: `cargo test`
Expected: tous les tests réussis, y compris les 3 de `tests/startup.rs`.

- [ ] **Step 5: Vérifications obligatoires**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe.

- [ ] **Step 6: Vérification visuelle de l'écran**

`owl` est une interface en terminal, pas une page web : `claude-in-chrome` ne peut pas la rendre et il n'y a aucun compte de test à ouvrir. On capture donc l'écran réel avec `script`, comme à la tâche 3, et on contrôle son contenu.

Run:

```bash
export GITHUB_TOKEN=$(gh auth token)
cargo build --quiet
printf 'rq' | script -q /dev/null ./target/debug/owl > /tmp/owl-vue.txt 2>&1
echo "code de sortie : $?"
grep -c 'owl — pull requests' /tmp/owl-vue.txt
grep -c 'Aucune pull request' /tmp/owl-vue.txt
grep -c 'q quitter · r rafraîchir' /tmp/owl-vue.txt
grep -c 'mis à jour à' /tmp/owl-vue.txt
grep -c '1049l' /tmp/owl-vue.txt
```

Attendu : code de sortie 0 et tous les `grep` à au moins 1. Cela prouve, dans l'ordre : le cadre titré, la liste vide annoncée par `app`, la barre d'aide, l'heure du dernier rafraîchissement, et la fermeture propre après « q » — la touche « r » ayant été traitée avant sans casser la boucle.

Puis lire `/tmp/owl-vue.txt` pour contrôler à l'œil qu'il n'y a ni texte tronqué au hasard, ni ligne dupliquée, ni accent cassé. Si un point échoue, corriger avant de continuer.

- [ ] **Step 7: Noter les bouchons dans les specs concernées**

Les fondations laissent quatre choses volontairement inachevées. Chaque spec concernée doit le dire, pour que personne ne prenne un bouchon pour du définitif.

Dans `docs/specs/01-modele-et-donnees.md`, ajouter à la fin, avant les critères de réussite :

```markdown
## Note d'implémentation

Les fondations laissent un bouchon : `github::fetch_pull_requests` renvoie une
liste vide sans toucher au réseau, et son erreur est un simple `String`. Cette
spec remplace le corps de la fonction et substitue au `String` un type d'erreur
`thiserror`. Le type `app::Event::Data` change en conséquence.
```

Dans `docs/specs/02-filtres.md`, ajouter à la fin, avant les critères de réussite :

```markdown
## Note d'implémentation

Les fondations laissent `filter.rs` vide : les filtres circulent sous forme de
chaînes, telles qu'écrites dans le fichier de réglages, et `config::Config`
expose `filters: Vec<String>`. Cette spec introduit `Filter` et `build_query`,
et c'est `app` qui traduit les chaînes des réglages en variantes de `Filter`.
```

Dans `docs/specs/03-affichage-et-navigation.md`, ajouter à la fin, avant les critères de réussite :

```markdown
## Note d'implémentation

Les fondations laissent une seule vue et un clavier réduit : `app::Key` ne
distingue que `Char` et `Other`, `ui::draw` dessine toujours la liste, et
`ui/detail.rs` est vide. Cette spec étend `Key` aux flèches et aux touches
d'action, ajoute la vue courante à `App`, et fait de `ui::draw` un véritable
aiguillage.
```

Dans `docs/specs/04-fusion.md`, ajouter à la fin, avant les critères de réussite :

```markdown
## Note d'implémentation

Les fondations lisent `preferred_merge_method` dans les réglages mais ne s'en
servent pas encore : le champ porte un `#[allow(dead_code)]` dans
`config::Config`. Cette spec l'utilise et retire l'attribut. `ui/merge.rs` est
vide jusque-là.
```

- [ ] **Step 8: Vérifications obligatoires, une dernière fois**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: tout passe.

- [ ] **Step 9: Commit**

```bash
git add src/main.rs src/ui docs/specs
git commit -m "Ajoute la boucle d'événements à trois producteurs et le dessin de la liste"
```

---

## Ce que ce plan ne fait pas

Volontairement hors périmètre des fondations, traité par les specs suivantes :

- Le vrai client GraphQL, les requêtes et le mappage des réponses (`01`).
- Le type `Filter` et la construction de la requête de recherche (`02`).
- La navigation, la vue détail, la sélection, le terminal trop étroit (`03`).
- La fusion et sa fenêtre de confirmation (`04`).
- Les erreurs 401 et 403, la limite d'appels, le client testé contre un serveur local (`05`).
