//! Sous-commandes non interactives de `owl`, lues par l'app de la barre des
//! menus. Chacune écrit un seul document JSON et se termine ; l'écran n'est
//! jamais pris.

mod output;

use std::fmt;
use std::process::ExitCode;

use crate::model::{MergeMethod, PrKey};

/// Ce que demande la ligne de commande, quand elle demande autre chose que
/// la TUI.
#[derive(Debug, PartialEq)]
pub enum Request {
    List,
    Detail(PrKey),
    Merge(PrKey, MergeMethod),
}

const USAGE: &str = "Usage: owl [list | detail <owner/repo> <number> | merge <owner/repo> <number> <merge|squash|rebase>]
  owl list
  owl detail <owner/repo> <number>
  owl merge <owner/repo> <number> <merge|squash|rebase>";

/// Arguments incompréhensibles. Le texte d'aide est le seul message : il dit
/// à lui seul ce qui était attendu.
#[derive(Debug, PartialEq)]
pub struct UsageError;

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(USAGE)
    }
}

/// Lit les arguments, nom du programme exclu. `Ok(None)` : aucun argument,
/// c'est la TUI qui démarre.
pub fn parse(args: &[String]) -> Result<Option<Request>, UsageError> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Ok(None),
        ["list"] => Ok(Some(Request::List)),
        ["detail", repo, number] => Ok(Some(Request::Detail(parse_key(repo, number)?))),
        ["merge", repo, number, method] => Ok(Some(Request::Merge(
            parse_key(repo, number)?,
            parse_method(method)?,
        ))),
        _ => Err(UsageError),
    }
}

/// `org/dépôt` avec exactement une barre et deux moitiés non vides, et un
/// numéro strictement positif.
fn parse_key(repo: &str, number: &str) -> Result<PrKey, UsageError> {
    let mut parts = repo.split('/');
    let valid = matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty()
    );
    let number: u32 = number.parse().map_err(|_| UsageError)?;
    if !valid || number == 0 {
        return Err(UsageError);
    }
    Ok(PrKey {
        repo: repo.to_string(),
        number,
    })
}

fn parse_method(word: &str) -> Result<MergeMethod, UsageError> {
    match word {
        "merge" => Ok(MergeMethod::Merge),
        "squash" => Ok(MergeMethod::Squash),
        "rebase" => Ok(MergeMethod::Rebase),
        _ => Err(UsageError),
    }
}

/// Exécute la demande et rend le code de sortie. Complété à la tâche 3.
pub fn run(request: Request) -> ExitCode {
    let _ = request;
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MergeMethod, PrKey};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    fn key() -> PrKey {
        PrKey {
            repo: "moi/owl".to_string(),
            number: 42,
        }
    }

    #[test]
    fn no_argument_means_the_tui() {
        assert_eq!(parse(&args(&[])), Ok(None));
    }

    #[test]
    fn list_takes_no_argument() {
        assert_eq!(parse(&args(&["list"])), Ok(Some(Request::List)));
        assert_eq!(parse(&args(&["list", "extra"])), Err(UsageError));
    }

    #[test]
    fn detail_takes_a_repo_and_a_number() {
        assert_eq!(
            parse(&args(&["detail", "moi/owl", "42"])),
            Ok(Some(Request::Detail(key())))
        );
    }

    #[test]
    fn merge_takes_a_repo_a_number_and_a_method() {
        for (word, method) in [
            ("merge", MergeMethod::Merge),
            ("squash", MergeMethod::Squash),
            ("rebase", MergeMethod::Rebase),
        ] {
            assert_eq!(
                parse(&args(&["merge", "moi/owl", "42", word])),
                Ok(Some(Request::Merge(key(), method)))
            );
        }
    }

    #[test]
    fn malformed_arguments_are_a_usage_error() {
        for bad in [
            &["detail"][..],
            &["detail", "moi/owl"],
            &["detail", "owl", "42"],
            &["detail", "/owl", "42"],
            &["detail", "moi/", "42"],
            &["detail", "moi/owl/x", "42"],
            &["detail", "moi/owl", "abc"],
            &["detail", "moi/owl", "0"],
            &["detail", "moi/owl", "-3"],
            &["merge", "moi/owl", "42"],
            &["merge", "moi/owl", "42", "fast-forward"],
            &["unknown"],
        ] {
            assert_eq!(parse(&args(bad)), Err(UsageError), "{bad:?}");
        }
    }

    #[test]
    fn the_usage_message_names_every_subcommand() {
        let text = UsageError.to_string();
        assert!(text.contains("owl list"));
        assert!(text.contains("owl detail <owner/repo> <number>"));
        assert!(text.contains("owl merge <owner/repo> <number> <merge|squash|rebase>"));
    }
}
