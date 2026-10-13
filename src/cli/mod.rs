//! Sous-commandes non interactives de `owl`, lues par l'app de la barre des
//! menus. Chacune écrit un seul document JSON et se termine ; l'écran n'est
//! jamais pris.

mod output;

use std::fmt;
use std::process::ExitCode;

use serde::Serialize;

use crate::config::{self, Config};
use crate::filter::{self, Filter};
use crate::github::{Client, GithubError};
use crate::model::{ListPage, MergeMethod, PrKey, PrSummary};
use crate::token;

use output::{ErrorBody, MergeOut};

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

/// Exécute la demande, écrit l'enveloppe sur la sortie standard et rend le
/// code de sortie : 0 si `ok`, 1 sinon. Toute erreur passe par l'enveloppe,
/// réglages et jeton compris : l'app n'a qu'un format à lire.
pub fn run(request: Request) -> ExitCode {
    let (text, ok) = match request {
        Request::List => output::envelope(perform(|settings, client| async move {
            let page = client
                .fetch_pull_requests(&query(&settings), settings.page_size)
                .await
                .map_err(|error| ErrorBody::from(&error))?;
            Ok(output::list_data(&page, &settings))
        })),
        Request::Detail(key) => output::envelope(perform(|settings, client| async move {
            let summary = summary_of(&settings, &client, &key).await?;
            let detail = client
                .fetch_detail(&summary)
                .await
                .map_err(|error| ErrorBody::from(&error))?;
            Ok(output::detail_data(&detail))
        })),
        Request::Merge(key, method) => output::envelope(perform(|settings, client| async move {
            let summary = summary_of(&settings, &client, &key).await?;
            client
                .merge_pull_request(&summary, None, method)
                .await
                .map_err(|error| ErrorBody::from(&error))?;
            Ok(MergeOut {})
        })),
    };
    println!("{text}");
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Prépare réglages, jeton et client, puis lance le travail dans un
/// exécuteur. Le jeton entre dans le client et n'en ressort pas.
fn perform<T, F, Fut>(work: F) -> Result<T, ErrorBody>
where
    T: Serialize,
    F: FnOnce(Config, Client) -> Fut,
    Fut: std::future::Future<Output = Result<T, ErrorBody>>,
{
    let settings = config::load().map_err(|error| ErrorBody::from(&error))?;
    let token = token::resolve().map_err(|error| ErrorBody::from(&error))?;
    let client = Client::new(token.expose()).map_err(|error| ErrorBody::from(&error))?;
    let runtime =
        tokio::runtime::Runtime::new().map_err(|_| ErrorBody::from(&GithubError::Transport))?;
    runtime.block_on(work(settings, client))
}

/// Chaîne de recherche, assemblée comme la TUI l'assemble.
fn query(settings: &Config) -> String {
    let filters: Vec<Filter> = settings
        .filters
        .iter()
        .map(|text| Filter::parse(text))
        .collect();
    filter::build_query(&filters)
}

/// Relance la liste et y cherche la clé : `github` attend un résumé, et
/// l'app ne montre que des pull requests de cette liste.
async fn summary_of(
    settings: &Config,
    client: &Client,
    key: &PrKey,
) -> Result<PrSummary, ErrorBody> {
    let page = client
        .fetch_pull_requests(&query(settings), settings.page_size)
        .await
        .map_err(|error| ErrorBody::from(&error))?;
    find(page, key).map_err(|error| ErrorBody::from(&error))
}

/// La pull request de cette clé dans la liste, ou `NotFound` si elle en est
/// sortie.
fn find(page: ListPage, key: &PrKey) -> Result<PrSummary, GithubError> {
    page.pull_requests
        .into_iter()
        .find(|pr| &pr.key == key)
        .ok_or(GithubError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::GithubError;
    use crate::model::ListPage;
    use crate::model::{MergeMethod, PrKey};

    fn recorded_page() -> ListPage {
        #[derive(serde::Deserialize)]
        struct Envelope {
            data: crate::github::dto::ListData,
        }
        serde_json::from_str::<Envelope>(include_str!("../../tests/fixtures/list.json"))
            .expect("la réponse enregistrée doit se lire")
            .data
            .to_list_page()
    }

    #[test]
    fn find_returns_the_summary_with_that_key() {
        let summary = find(recorded_page(), &key()).expect("présente");
        assert_eq!(summary.key, key());
        assert_eq!(summary.title, "Ajoute la fenêtre de fusion");
    }

    #[test]
    fn a_key_outside_the_list_is_not_found() {
        let missing = PrKey {
            repo: "moi/owl".to_string(),
            number: 9999,
        };
        assert!(matches!(
            find(recorded_page(), &missing),
            Err(GithubError::NotFound)
        ));
    }

    #[test]
    fn the_query_is_built_from_the_settings() {
        let settings = crate::config::Config::default();
        assert_eq!(
            query(&settings),
            crate::filter::build_query(
                &settings
                    .filters
                    .iter()
                    .map(|text| crate::filter::Filter::parse(text))
                    .collect::<Vec<_>>()
            )
        );
    }

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
