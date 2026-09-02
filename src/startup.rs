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
        Err(erreur @ (GithubError::Unauthorized | GithubError::Forbidden)) => {
            FirstResponse::Fatal(erreur.to_string())
        }
        autre => FirstResponse::Start(autre),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rend le message d'une réponse fatale, ou échoue si elle ne l'est pas.
    fn message_fatal(reponse: FirstResponse<u8>) -> String {
        match reponse {
            FirstResponse::Fatal(message) => message,
            FirstResponse::Start(_) => panic!("cette réponse devait être fatale"),
        }
    }

    /// Vrai si la réponse laisse démarrer.
    fn demarre(reponse: FirstResponse<u8>) -> bool {
        matches!(reponse, FirstResponse::Start(_))
    }

    #[test]
    fn un_jeton_refuse_empeche_le_demarrage() {
        let message = message_fatal(classify::<u8>(Err(GithubError::Unauthorized)));
        assert_eq!(
            message,
            "Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler."
        );
    }

    #[test]
    fn des_droits_insuffisants_empechent_le_demarrage() {
        let message = message_fatal(classify::<u8>(Err(GithubError::Forbidden)));
        assert_eq!(
            message,
            "Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`."
        );
    }

    #[test]
    fn un_reseau_injoignable_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Err(GithubError::Transport))));
    }

    #[test]
    fn une_limite_d_appels_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Err(GithubError::RateLimited {
            reset_at: None
        }))));
    }

    #[test]
    fn une_reponse_illisible_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Err(GithubError::Malformed))));
    }

    #[test]
    fn une_reponse_reussie_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Ok(7))));
    }
}
