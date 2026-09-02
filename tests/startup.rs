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
