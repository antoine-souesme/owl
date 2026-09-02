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
    // Pas encore appelée hors tests : l'en-tête HTTP arrive avec la spec 03
    // (réseau). Sans ce `allow`, clippy la signale comme morte.
    #[allow(dead_code)]
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
        let resultat = resolve_from(None, Some("jeton-github".into()), gh_ok("jeton-gh")).unwrap();
        assert_eq!(resultat.expose(), "jeton-github");
    }

    #[test]
    fn gh_utilise_en_dernier_recours() {
        let resultat = resolve_from(None, None, gh_ok("jeton-gh")).unwrap();
        assert_eq!(resultat.expose(), "jeton-gh");
    }

    #[test]
    fn une_variable_vide_compte_comme_absente() {
        let resultat =
            resolve_from(Some("   ".into()), Some("jeton-github".into()), gh_ok("x")).unwrap();
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
