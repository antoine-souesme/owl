//! Lecture du fichier de réglages `~/.config/owl/config.toml`.
//!
//! Le fichier est optionnel. Une clé inconnue est ignorée. Une valeur invalide
//! arrête le programme avec un message qui nomme la clé fautive.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Méthode de fusion présélectionnée quand le dépôt en autorise plusieurs.
/// Le type appartient au modèle : `github` en a besoin pour la mutation, et
/// n'a pas le droit de dépendre des réglages.
pub use crate::model::MergeMethod;

/// Réglages effectifs du programme.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Filtres actifs au démarrage, dans la syntaxe de recherche de GitHub.
    pub filters: Vec<String>,
    /// Intervalle de rafraîchissement en secondes. 0 désactive le minuteur.
    pub refresh_interval: u64,
    /// Méthode de fusion préférée. Utilisée par `04-fusion.md`.
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
    #[error("Réglages invalides dans {path} : fichier illisible.")]
    Unreadable { path: String },
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
    Ok(base
        .home_dir()
        .join(".config")
        .join("owl")
        .join("config.toml"))
}

/// Lit les réglages au chemin par défaut.
pub fn load() -> Result<Config, ConfigError> {
    load_from(&default_path()?)
}

/// Lit les réglages à un chemin donné. Fichier absent : valeurs par défaut,
/// parce que la spec rend le fichier optionnel. Fichier présent mais illisible
/// (droits insuffisants, par exemple) : erreur, car c'est une vraie
/// mauvaise configuration à signaler, pas une absence.
pub fn load_from(path: &Path) -> Result<Config, ConfigError> {
    let affichage = path.display().to_string();

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Config::default());
        }
        Err(_) => {
            return Err(ConfigError::Unreadable { path: affichage });
        }
    };

    let document: toml::Table = toml::from_str(&text).map_err(|_| ConfigError::Syntax {
        path: affichage.clone(),
    })?;

    let mut settings = Config::default();

    let invalide = |key: &str| ConfigError::InvalidKey {
        path: affichage.clone(),
        key: key.to_string(),
    };

    if let Some(value) = document.get("filters") {
        let list = value.as_array().ok_or_else(|| invalide("filters"))?;
        let mut filters = Vec::with_capacity(list.len());
        for element in list {
            let text = element.as_str().ok_or_else(|| invalide("filters"))?;
            filters.push(text.to_string());
        }
        // Une liste de chaînes blanches ne vaut pas mieux qu'une liste vide :
        // les fragments vides sont écartés de la requête, qui ramènerait
        // alors tout GitHub.
        if filters.iter().all(|filter| filter.trim().is_empty()) {
            return Err(ConfigError::EmptyFilters);
        }
        settings.filters = filters;
    }

    if let Some(value) = document.get("refresh_interval") {
        let seconds = value
            .as_integer()
            .ok_or_else(|| invalide("refresh_interval"))?;
        settings.refresh_interval =
            u64::try_from(seconds).map_err(|_| invalide("refresh_interval"))?;
    }

    if let Some(value) = document.get("preferred_merge_method") {
        let text = value
            .as_str()
            .ok_or_else(|| invalide("preferred_merge_method"))?;
        settings.preferred_merge_method = match text {
            "squash" => MergeMethod::Squash,
            "rebase" => MergeMethod::Rebase,
            "merge" => MergeMethod::Merge,
            _ => return Err(invalide("preferred_merge_method")),
        };
    }

    if let Some(value) = document.get("page_size") {
        let count = value.as_integer().ok_or_else(|| invalide("page_size"))?;
        if !(1..=100).contains(&count) {
            return Err(invalide("page_size"));
        }
        settings.page_size = count as u16;
    }

    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Écrit un fichier de réglages temporaire et le lit.
    fn read(content: &str) -> Result<Config, ConfigError> {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        load_from(file.path())
    }

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let settings = load_from(Path::new("/introuvable/owl/config.toml")).unwrap();
        assert_eq!(settings, Config::default());
    }

    #[test]
    fn the_defaults_match_the_spec() {
        let settings = Config::default();
        assert_eq!(settings.filters, vec!["author:@me", "is:open"]);
        assert_eq!(settings.refresh_interval, 60);
        assert_eq!(settings.preferred_merge_method, MergeMethod::Squash);
        assert_eq!(settings.page_size, 50);
    }

    #[test]
    fn an_empty_file_gives_the_defaults() {
        assert_eq!(read("").unwrap(), Config::default());
    }

    #[test]
    fn a_complete_file_is_read_in_full() {
        let settings = read(
            r#"
filters = ["review-requested:@me"]
refresh_interval = 0
preferred_merge_method = "rebase"
page_size = 100
"#,
        )
        .unwrap();
        assert_eq!(settings.filters, vec!["review-requested:@me"]);
        assert_eq!(settings.refresh_interval, 0);
        assert_eq!(settings.preferred_merge_method, MergeMethod::Rebase);
        assert_eq!(settings.page_size, 100);
    }

    #[test]
    fn an_unknown_key_is_ignored_without_error() {
        let settings = read("couleur_preferee = \"bleu\"\nrefresh_interval = 30\n").unwrap();
        assert_eq!(settings.refresh_interval, 30);
    }

    #[test]
    fn the_three_merge_methods_are_accepted() {
        for (text, expected) in [
            ("squash", MergeMethod::Squash),
            ("rebase", MergeMethod::Rebase),
            ("merge", MergeMethod::Merge),
        ] {
            let settings = read(&format!("preferred_merge_method = \"{text}\"\n")).unwrap();
            assert_eq!(settings.preferred_merge_method, expected);
        }
    }

    #[test]
    fn an_unknown_merge_method_is_refused_with_its_key() {
        let error = read("preferred_merge_method = \"fast-forward\"\n").unwrap_err();
        let message = error.to_string();
        assert!(message.starts_with("Réglages invalides dans "), "{message}");
        assert!(message.ends_with(" : preferred_merge_method."), "{message}");
    }

    #[test]
    fn an_out_of_range_page_size_is_refused_with_its_key() {
        for value in ["0", "101", "-5"] {
            let error = read(&format!("page_size = {value}\n")).unwrap_err();
            assert!(
                error.to_string().ends_with(" : page_size."),
                "valeur {value} → {error}"
            );
        }
    }

    #[test]
    fn a_wrong_type_is_refused_with_its_key() {
        let error = read("page_size = \"beaucoup\"\n").unwrap_err();
        assert!(error.to_string().ends_with(" : page_size."), "{error}");

        let error = read("filters = \"author:@me\"\n").unwrap_err();
        assert!(error.to_string().ends_with(" : filters."), "{error}");

        let error = read("refresh_interval = -1\n").unwrap_err();
        assert!(
            error.to_string().ends_with(" : refresh_interval."),
            "{error}"
        );
    }

    #[test]
    fn an_empty_filter_list_is_refused_with_its_own_message() {
        let error = read("filters = []\n").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Aucun filtre actif : la recherche ramènerait tout GitHub."
        );
    }

    #[test]
    fn an_all_blank_filter_list_is_refused_like_an_empty_one() {
        let error = read("filters = [\"\", \"   \"]\n").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Aucun filtre actif : la recherche ramènerait tout GitHub."
        );
    }

    #[test]
    fn invalid_toml_syntax_is_refused_with_the_path() {
        let error = read("filters = [\n").unwrap_err();
        let message = error.to_string();
        assert!(message.starts_with("Réglages invalides dans "), "{message}");
        assert!(message.ends_with(" : syntaxe TOML invalide."), "{message}");
    }

    #[test]
    fn the_default_path_is_under_config_owl() {
        let path = default_path().unwrap();
        assert!(
            path.ends_with("owl/config.toml"),
            "chemin = {}",
            path.display()
        );
    }

    #[test]
    #[cfg(unix)]
    fn an_unreadable_file_is_refused_with_its_path() {
        use std::os::unix::fs::PermissionsExt;

        if unsafe { libc_geteuid() } == 0 {
            // Root ignore les permissions de lecture : le test n'aurait rien
            // à vérifier dans ce contexte (CI lancée en root, par exemple).
            return;
        }

        let file = NamedTempFile::new().unwrap();
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o000)).unwrap();

        let error = load_from(file.path()).unwrap_err();
        let message = error.to_string();
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o600)).unwrap();

        assert!(message.starts_with("Réglages invalides dans "), "{message}");
        assert!(message.ends_with(" : fichier illisible."), "{message}");
    }

    #[cfg(unix)]
    unsafe fn libc_geteuid() -> u32 {
        extern "C" {
            fn geteuid() -> u32;
        }
        geteuid()
    }

    #[test]
    fn a_missing_home_directory_has_its_own_message() {
        assert_eq!(
            ConfigError::NoHomeDirectory.to_string(),
            "Impossible de déterminer le dossier de configuration."
        );
    }
}
