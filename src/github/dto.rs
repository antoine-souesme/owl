//! Types de réponse brute de l'API, mappés vers `model`.
//!
//! Seul endroit du programme qui connaît le vocabulaire de GitHub. Tous les
//! champs sont optionnels dès que GitHub peut ne rien renvoyer : `search` de
//! type `ISSUE` ramène aussi des issues, dont le nœud n'a aucun des champs
//! d'une pull request.

// Les tâches suivantes et la spec 03 consomment le reste de ces types.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::model::{
    ChecksState, ListPage, MergeableState, PrKey, PrSummary, RateLimit, RepoMergeRules,
    ReviewState, AUTEUR_INCONNU,
};

/// Contenu du champ `data` de la requête de liste.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListData {
    pub search: Search,
    pub rate_limit: Option<RateLimitDto>,
}

#[derive(Debug, Deserialize)]
pub struct Search {
    /// Un nœud peut être `null` : GraphQL autorise l'absence.
    pub nodes: Vec<Option<SearchNode>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchNode {
    pub number: Option<u32>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub is_draft: Option<bool>,
    pub mergeable: Option<String>,
    pub review_decision: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub author: Option<Actor>,
    pub repository: Option<RepositoryDto>,
    pub commits: Option<CommitConnection>,
}

#[derive(Debug, Deserialize)]
pub struct Actor {
    pub login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDto {
    pub name_with_owner: String,
    pub squash_merge_allowed: bool,
    pub merge_commit_allowed: bool,
    pub rebase_merge_allowed: bool,
    pub delete_branch_on_merge: bool,
}

#[derive(Debug, Deserialize)]
pub struct CommitConnection {
    pub nodes: Vec<CommitNode>,
}

#[derive(Debug, Deserialize)]
pub struct CommitNode {
    pub commit: Commit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub status_check_rollup: Option<Rollup>,
}

#[derive(Debug, Deserialize)]
pub struct Rollup {
    pub state: Option<String>,
    /// Rempli par la requête de détail seulement.
    pub contexts: Option<ContextConnection>,
}

/// Vide à cette tâche : la requête de liste ne demande pas les contextes.
/// La tâche 4 la remplit.
#[derive(Debug, Deserialize)]
pub struct ContextConnection {
    pub nodes: Vec<Option<ContextNode>>,
}

#[derive(Debug, Deserialize)]
pub struct ContextNode {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitDto {
    pub remaining: u32,
    pub reset_at: DateTime<Utc>,
}

impl ListData {
    /// Traduit la réponse en types métier.
    pub fn to_list_page(&self) -> ListPage {
        ListPage {
            pull_requests: self
                .search
                .nodes
                .iter()
                .flatten()
                .filter_map(SearchNode::to_summary)
                .collect(),
            rate_limit: self.rate_limit.as_ref().map(|limite| RateLimit {
                remaining: limite.remaining,
                reset_at: limite.reset_at,
            }),
        }
    }
}

impl SearchNode {
    /// `None` quand le nœud ne porte pas les champs d'une pull request : c'est
    /// une issue ramenée par `search`, ignorée silencieusement.
    pub fn to_summary(&self) -> Option<PrSummary> {
        let number = self.number?;
        let title = self.title.clone()?;
        let url = self.url.clone()?;
        let updated_at = self.updated_at?;
        let repository = self.repository.as_ref()?;

        Some(PrSummary {
            key: PrKey {
                repo: repository.name_with_owner.clone(),
                number,
            },
            title,
            author: self
                .author
                .as_ref()
                .map(|auteur| auteur.login.clone())
                .unwrap_or_else(|| AUTEUR_INCONNU.to_string()),
            url,
            is_draft: self.is_draft.unwrap_or(false),
            checks: rollup_state(self.commits.as_ref()),
            review: review_from_decision(self.review_decision.as_deref()),
            mergeable: mergeable_from(self.mergeable.as_deref()),
            updated_at,
            repo_rules: RepoMergeRules {
                squash: repository.squash_merge_allowed,
                merge: repository.merge_commit_allowed,
                rebase: repository.rebase_merge_allowed,
                delete_branch_on_merge: repository.delete_branch_on_merge,
            },
        })
    }
}

/// État global des vérifications, lu sur le dernier commit. Aucun commit ou
/// aucun `statusCheckRollup` veut dire « aucune CI », donc `None` et non
/// `Pending`.
fn rollup_state(commits: Option<&CommitConnection>) -> ChecksState {
    let rollup = commits
        .and_then(|connexion| connexion.nodes.first())
        .and_then(|noeud| noeud.commit.status_check_rollup.as_ref());
    match rollup {
        Some(rollup) => checks_from_rollup(rollup.state.as_deref()),
        None => ChecksState::None,
    }
}

/// Table de la spec. Une valeur inconnue est traitée comme une absence : mieux
/// vaut ne rien annoncer qu'annoncer faux.
pub fn checks_from_rollup(state: Option<&str>) -> ChecksState {
    match state {
        Some("SUCCESS") => ChecksState::Success,
        Some("FAILURE") | Some("ERROR") => ChecksState::Failure,
        Some("PENDING") | Some("EXPECTED") => ChecksState::Pending,
        _ => ChecksState::None,
    }
}

fn review_from_decision(decision: Option<&str>) -> ReviewState {
    match decision {
        Some("APPROVED") => ReviewState::Approved,
        Some("CHANGES_REQUESTED") => ReviewState::ChangesRequested,
        Some("REVIEW_REQUIRED") => ReviewState::ReviewRequired,
        _ => ReviewState::None,
    }
}

fn mergeable_from(value: Option<&str>) -> MergeableState {
    match value {
        Some("MERGEABLE") => MergeableState::Mergeable,
        Some("CONFLICTING") => MergeableState::Conflicting,
        _ => MergeableState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChecksState, ListPage, MergeableState, PrKey, RepoMergeRules, ReviewState};
    use serde::Deserialize;

    /// Enveloppe de la réponse enregistrée, dont seul `data` nous intéresse.
    #[derive(Deserialize)]
    struct Enveloppe {
        data: ListData,
    }

    const REPONSE: &str = include_str!("../../tests/fixtures/list.json");

    fn page() -> ListPage {
        serde_json::from_str::<Enveloppe>(REPONSE)
            .expect("la réponse enregistrée doit se lire")
            .data
            .to_list_page()
    }

    #[test]
    fn la_reponse_enregistree_donne_la_premiere_pull_request_exacte() {
        let page = page();
        let premiere = &page.pull_requests[0];
        assert_eq!(
            premiere.key,
            PrKey {
                repo: "moi/owl".to_string(),
                number: 42
            }
        );
        assert_eq!(premiere.title, "Ajoute la fenêtre de fusion");
        assert_eq!(premiere.author, "moi");
        assert_eq!(premiere.url, "https://github.com/moi/owl/pull/42");
        assert!(!premiere.is_draft);
        assert_eq!(premiere.checks, ChecksState::Success);
        assert_eq!(premiere.review, ReviewState::Approved);
        assert_eq!(premiere.mergeable, MergeableState::Mergeable);
        assert_eq!(
            premiere.updated_at.to_rfc3339(),
            "2026-08-30T09:12:44+00:00"
        );
        assert_eq!(
            premiere.repo_rules,
            RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            }
        );
    }

    #[test]
    fn un_noeud_d_issue_est_ignore_sans_faire_echouer_la_traduction() {
        assert_eq!(
            page().pull_requests.len(),
            5,
            "six nœuds enregistrés, dont un qui n'est pas une pull request"
        );
    }

    #[test]
    fn une_pr_sans_aucune_ci_donne_none_et_non_pending() {
        let page = page();
        let sans_commit = &page.pull_requests[1];
        assert_eq!(sans_commit.key.number, 7);
        assert_eq!(sans_commit.checks, ChecksState::None);

        let sans_rollup = &page.pull_requests[4];
        assert_eq!(sans_rollup.key.number, 3);
        assert_eq!(sans_rollup.checks, ChecksState::None);
    }

    #[test]
    fn les_etats_sont_traduits_un_a_un() {
        let page = page();
        let etats: Vec<(ChecksState, ReviewState, MergeableState)> = page
            .pull_requests
            .iter()
            .map(|pr| (pr.checks, pr.review, pr.mergeable))
            .collect();
        assert_eq!(
            etats,
            vec![
                (
                    ChecksState::Success,
                    ReviewState::Approved,
                    MergeableState::Mergeable
                ),
                (
                    ChecksState::None,
                    ReviewState::None,
                    MergeableState::Unknown
                ),
                (
                    ChecksState::Pending,
                    ReviewState::ChangesRequested,
                    MergeableState::Conflicting
                ),
                (
                    ChecksState::Failure,
                    ReviewState::ReviewRequired,
                    MergeableState::Mergeable
                ),
                (
                    ChecksState::None,
                    ReviewState::None,
                    MergeableState::Mergeable
                ),
            ]
        );
    }

    #[test]
    fn un_auteur_supprime_devient_inconnu() {
        assert_eq!(page().pull_requests[1].author, "inconnu");
    }

    #[test]
    fn un_brouillon_est_marque_comme_tel() {
        let page = page();
        assert!(page.pull_requests[1].is_draft);
        assert!(!page.pull_requests[0].is_draft);
    }

    #[test]
    fn le_solde_d_appels_est_lu() {
        let limite = page().rate_limit.expect("le solde est présent");
        assert_eq!(limite.remaining, 4987);
        assert_eq!(limite.reset_at.to_rfc3339(), "2026-08-30T10:00:00+00:00");
    }
}
