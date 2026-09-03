//! Tests des erreurs de démarrage, sur le binaire réel.

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Lance `owl` sans aucune variable d'environnement et sans `gh` accessible.
fn owl_without_auth() -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_owl"))
        .env_clear()
        .env("PATH", "")
        .env("HOME", "/introuvable-owl")
        .output()
        .expect("le binaire owl doit être exécutable")
}

#[test]
fn without_a_token_or_gh_the_message_says_to_install_gh() {
    let output = owl_without_auth();
    let error = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        error.trim(),
        "owl a besoin de gh. Installe-le, puis lance `gh auth login`."
    );
}

#[test]
fn without_a_token_the_exit_code_is_non_zero() {
    let output = owl_without_auth();
    assert!(
        !output.status.success(),
        "code de sortie = {:?}",
        output.status.code()
    );
}

#[test]
fn without_a_token_nothing_is_written_to_standard_output() {
    let output = owl_without_auth();
    let standard = String::from_utf8_lossy(&output.stdout);
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
fn owl_with_settings(content: &str) -> (PathBuf, std::process::Output) {
    let maison = TempDir::new().expect("dossier temporaire");
    let directory = maison.path().join(".config").join("owl");
    std::fs::create_dir_all(&directory).expect("création du dossier de réglages");
    let file = directory.join("config.toml");
    std::fs::write(&file, content).expect("écriture des réglages");

    let output = Command::new(env!("CARGO_BIN_EXE_owl"))
        .env_clear()
        .env("PATH", "")
        .env("HOME", maison.path())
        .output()
        .expect("le binaire owl doit être exécutable");

    (file, output)
}

/// Vérifie la forme complète d'une erreur de démarrage : message exact sur la
/// sortie d'erreur, code non nul, et sortie standard restée vierge.
fn assert_startup_error(output: &std::process::Output, message: &str) {
    let error = String::from_utf8_lossy(&output.stderr);
    assert_eq!(error.trim(), message);
    assert!(
        !output.status.success(),
        "code de sortie = {:?}",
        output.status.code()
    );
    let standard = String::from_utf8_lossy(&output.stdout);
    assert!(
        standard.is_empty(),
        "aucune prise de contrôle du terminal : {standard:?}"
    );
}

#[test]
fn an_invalid_setting_value_names_its_key_and_its_path() {
    let (file, output) = owl_with_settings("page_size = 0\n");
    assert_startup_error(
        &output,
        &format!("Réglages invalides dans {} : page_size.", file.display()),
    );
}

#[test]
fn broken_toml_syntax_says_so_with_its_path() {
    let (file, output) = owl_with_settings("filters = [\n");
    assert_startup_error(
        &output,
        &format!(
            "Réglages invalides dans {} : syntaxe TOML invalide.",
            file.display()
        ),
    );
}

#[test]
fn an_empty_filter_list_has_its_own_message() {
    let (_, output) = owl_with_settings("filters = []\n");
    assert_startup_error(
        &output,
        "Aucun filtre actif : la recherche ramènerait tout GitHub.",
    );
}
