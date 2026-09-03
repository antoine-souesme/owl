//! Classement de la première réponse de GitHub, reçue avant toute prise de
//! contrôle du terminal.
//!
//! Deux refus de GitHub sont des erreurs de démarrage et non des messages de
//! barre d'état : un jeton refusé et des droits insuffisants ne se corrigent
//! pas en attendant le prochain rafraîchissement. Tout le reste — réseau
//! injoignable, limite d'appels, réponse illisible — laisse `owl` démarrer :
//! la liste s'affichera vide, et une limite d'appels suspendra tout de suite
//! le rafraîchissement au lieu de laisser l'erreur en barre d'état.

use crate::github::GithubError;

/// Ce que devient la première réponse de GitHub.
pub enum FirstResponse<T> {
    /// `owl` peut démarrer, avec ce résultat comme premier événement.
    Start(Result<T, GithubError>),
    /// Erreur de démarrage : message sur la sortie d'erreur, code non nul.
    Fatal(String),
}

pub fn classify<T>(result: Result<T, GithubError>) -> FirstResponse<T> {
    match result {
        Err(error @ (GithubError::Unauthorized | GithubError::Forbidden)) => {
            FirstResponse::Fatal(error.to_string())
        }
        other => FirstResponse::Start(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rend le message d'une réponse fatale, ou échoue si elle ne l'est pas.
    fn fatal_message(response: FirstResponse<u8>) -> String {
        match response {
            FirstResponse::Fatal(message) => message,
            FirstResponse::Start(_) => panic!("cette réponse devait être fatale"),
        }
    }

    /// Vrai si la réponse laisse démarrer.
    fn starts(response: FirstResponse<u8>) -> bool {
        matches!(response, FirstResponse::Start(_))
    }

    #[test]
    fn a_refused_token_prevents_startup() {
        let message = fatal_message(classify::<u8>(Err(GithubError::Unauthorized)));
        assert_eq!(
            message,
            "Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler."
        );
    }

    #[test]
    fn insufficient_permissions_prevent_startup() {
        let message = fatal_message(classify::<u8>(Err(GithubError::Forbidden)));
        assert_eq!(
            message,
            "Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`."
        );
    }

    #[test]
    fn an_unreachable_network_still_lets_owl_start() {
        assert!(starts(classify::<u8>(Err(GithubError::Transport))));
    }

    #[test]
    fn a_rate_limit_still_lets_owl_start() {
        assert!(starts(classify::<u8>(Err(GithubError::RateLimited {
            reset_at: None
        }))));
    }

    #[test]
    fn an_unreadable_response_still_lets_owl_start() {
        assert!(starts(classify::<u8>(Err(GithubError::Malformed))));
    }

    #[test]
    fn a_successful_response_lets_owl_start() {
        assert!(starts(classify::<u8>(Ok(7))));
    }
}
