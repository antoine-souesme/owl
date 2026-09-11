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
        f.write_str("Token(hidden)")
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
    #[error("owl needs gh. Install it, then run `gh auth login`.")]
    GhMissing,
    #[error("Not signed in to GitHub. Run `gh auth login`.")]
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
    if let Some(value) = owl.and_then(not_empty) {
        return Ok(Token(value));
    }
    if let Some(value) = github.and_then(not_empty) {
        return Ok(Token(value));
    }
    match gh() {
        Ok(output) => not_empty(output)
            .map(Token)
            .ok_or(TokenError::GhNotAuthenticated),
        Err(GhFailure::NotFound) => Err(TokenError::GhMissing),
        Err(GhFailure::NotAuthenticated) => Err(TokenError::GhNotAuthenticated),
    }
}

/// Rend `None` pour une chaîne vide ou faite d'espaces.
fn not_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Lance `gh auth token`. Un `gh` introuvable et un `gh` déconnecté sont deux
/// situations différentes, avec deux messages différents.
fn run_gh_auth_token() -> Result<String, GhFailure> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|_| GhFailure::NotFound)?;

    if !output.status.success() {
        return Err(GhFailure::NotAuthenticated);
    }

    String::from_utf8(output.stdout).map_err(|_| GhFailure::NotAuthenticated)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fabrique un appel à `gh` factice qui réussit.
    fn gh_ok(output: &str) -> impl FnOnce() -> Result<String, GhFailure> + '_ {
        move || Ok(output.to_string())
    }

    #[test]
    fn owl_token_wins_over_everything_else() {
        let result = resolve_from(
            Some("jeton-owl".into()),
            Some("jeton-github".into()),
            gh_ok("jeton-gh"),
        )
        .unwrap();
        assert_eq!(result.expose(), "jeton-owl");
    }

    #[test]
    fn github_token_is_used_when_owl_token_is_missing() {
        let result = resolve_from(None, Some("jeton-github".into()), gh_ok("jeton-gh")).unwrap();
        assert_eq!(result.expose(), "jeton-github");
    }

    #[test]
    fn gh_is_used_as_a_last_resort() {
        let result = resolve_from(None, None, gh_ok("jeton-gh")).unwrap();
        assert_eq!(result.expose(), "jeton-gh");
    }

    #[test]
    fn an_empty_variable_counts_as_missing() {
        let result =
            resolve_from(Some("   ".into()), Some("jeton-github".into()), gh_ok("x")).unwrap();
        assert_eq!(result.expose(), "jeton-github");
    }

    #[test]
    fn the_gh_output_is_trimmed() {
        let result = resolve_from(None, None, gh_ok("  jeton-gh\n")).unwrap();
        assert_eq!(result.expose(), "jeton-gh");
    }

    #[test]
    fn a_missing_gh_gives_the_install_message() {
        let error = resolve_from(None, None, || Err(GhFailure::NotFound)).unwrap_err();
        assert_eq!(
            error.to_string(),
            "owl needs gh. Install it, then run `gh auth login`."
        );
    }

    #[test]
    fn a_logged_out_gh_gives_the_login_message() {
        let error = resolve_from(None, None, || Err(GhFailure::NotAuthenticated)).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Not signed in to GitHub. Run `gh auth login`."
        );
    }

    #[test]
    fn a_gh_returning_nothing_counts_as_logged_out() {
        let error = resolve_from(None, None, gh_ok("\n")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Not signed in to GitHub. Run `gh auth login`."
        );
    }

    #[test]
    fn the_token_does_not_leak_through_its_debug() {
        let token = resolve_from(Some("ghp_secret".into()), None, gh_ok("x")).unwrap();
        let debug_output = format!("{token:?}");
        assert!(
            !debug_output.contains("ghp_secret"),
            "trace = {debug_output}"
        );
        assert_eq!(debug_output, "Token(hidden)");
    }
}
