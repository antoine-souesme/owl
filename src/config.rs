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

    let texte = match std::fs::read_to_string(path) {
        Ok(texte) => texte,
        Err(erreur) if erreur.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Config::default());
        }
        Err(_) => {
            return Err(ConfigError::Unreadable { path: affichage });
        }
    };

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
        // Une liste de chaînes blanches ne vaut pas mieux qu'une liste vide :
        // les fragments vides sont écartés de la requête, qui ramènerait
        // alors tout GitHub.
        if filtres.iter().all(|filtre| filtre.trim().is_empty()) {
            return Err(ConfigError::EmptyFilters);
        }
        reglages.filters = filtres;
    }

    if let Some(valeur) = table.get("refresh_interval") {
        let secondes = valeur
            .as_integer()
            .ok_or_else(|| invalide("refresh_interval"))?;
        reglages.refresh_interval =
            u64::try_from(secondes).map_err(|_| invalide("refresh_interval"))?;
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
    fn liste_de_filtres_toute_blanche_refusee_comme_une_liste_vide() {
        let erreur = lire("filters = [\"\", \"   \"]\n").unwrap_err();
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

    #[test]
    #[cfg(unix)]
    fn fichier_illisible_refuse_avec_son_chemin() {
        use std::os::unix::fs::PermissionsExt;

        if unsafe { libc_geteuid() } == 0 {
            // Root ignore les permissions de lecture : le test n'aurait rien
            // à vérifier dans ce contexte (CI lancée en root, par exemple).
            return;
        }

        let fichier = NamedTempFile::new().unwrap();
        std::fs::set_permissions(fichier.path(), std::fs::Permissions::from_mode(0o000)).unwrap();

        let erreur = load_from(fichier.path()).unwrap_err();
        let message = erreur.to_string();
        std::fs::set_permissions(fichier.path(), std::fs::Permissions::from_mode(0o600)).unwrap();

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
    fn le_dossier_personnel_introuvable_a_son_message() {
        assert_eq!(
            ConfigError::NoHomeDirectory.to_string(),
            "Impossible de déterminer le dossier de configuration."
        );
    }
}
