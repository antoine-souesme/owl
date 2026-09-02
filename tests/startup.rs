//! Tests des erreurs de démarrage, sur le binaire réel.

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

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

/// Écrit un fichier de réglages dans un dossier personnel temporaire, lance
/// `owl` dessus, et rend le chemin du fichier avec la sortie du programme.
fn owl_avec_reglages(contenu: &str) -> (PathBuf, std::process::Output) {
    let maison = TempDir::new().expect("dossier temporaire");
    let dossier = maison.path().join(".config").join("owl");
    std::fs::create_dir_all(&dossier).expect("création du dossier de réglages");
    let fichier = dossier.join("config.toml");
    std::fs::write(&fichier, contenu).expect("écriture des réglages");

    let sortie = Command::new(env!("CARGO_BIN_EXE_owl"))
        .env_clear()
        .env("PATH", "")
        .env("HOME", maison.path())
        .output()
        .expect("le binaire owl doit être exécutable");

    (fichier, sortie)
}

/// Vérifie la forme complète d'une erreur de démarrage : message exact sur la
/// sortie d'erreur, code non nul, et sortie standard restée vierge.
fn verifier_erreur_de_demarrage(sortie: &std::process::Output, message: &str) {
    let erreur = String::from_utf8_lossy(&sortie.stderr);
    assert_eq!(erreur.trim(), message);
    assert!(
        !sortie.status.success(),
        "code de sortie = {:?}",
        sortie.status.code()
    );
    let standard = String::from_utf8_lossy(&sortie.stdout);
    assert!(
        standard.is_empty(),
        "aucune prise de contrôle du terminal : {standard:?}"
    );
}

#[test]
fn une_valeur_de_reglage_invalide_nomme_sa_cle_et_son_chemin() {
    let (fichier, sortie) = owl_avec_reglages("page_size = 0\n");
    verifier_erreur_de_demarrage(
        &sortie,
        &format!("Réglages invalides dans {} : page_size.", fichier.display()),
    );
}

#[test]
fn une_syntaxe_toml_cassee_le_dit_avec_son_chemin() {
    let (fichier, sortie) = owl_avec_reglages("filters = [\n");
    verifier_erreur_de_demarrage(
        &sortie,
        &format!(
            "Réglages invalides dans {} : syntaxe TOML invalide.",
            fichier.display()
        ),
    );
}

#[test]
fn une_liste_de_filtres_vide_a_son_propre_message() {
    let (_, sortie) = owl_avec_reglages("filters = []\n");
    verifier_erreur_de_demarrage(
        &sortie,
        "Aucun filtre actif : la recherche ramènerait tout GitHub.",
    );
}
