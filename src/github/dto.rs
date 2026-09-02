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
    ChangedFile, CheckRun, ChecksState, Comment, ListPage, MergeableState, PrDetail, PrKey,
    PrSummary, RateLimit, RepoMergeRules, Review, ReviewState, AUTEUR_INCONNU,
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

/// Un contexte de vérification arrive sous deux formes, `CheckRun` (GitHub
/// Actions et équivalents) et `StatusContext` (anciens statuts d'API). Les
/// champs des deux fragments cohabitent ici, et `to_check_run` tranche.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextNode {
    // Forme `CheckRun`.
    pub name: Option<String>,
    pub conclusion: Option<String>,
    pub status: Option<String>,
    pub details_url: Option<String>,
    // Forme `StatusContext`.
    pub context: Option<String>,
    pub state: Option<String>,
    pub target_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitDto {
    pub remaining: u32,
    pub reset_at: DateTime<Utc>,
}

/// Contenu du champ `data` de la requête de détail.
#[derive(Debug, Deserialize)]
pub struct DetailData {
    pub repository: Option<RepositoryDetail>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDetail {
    pub pull_request: Option<PullRequestDetail>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestDetail {
    pub id: String,
    pub body: Option<String>,
    pub head_ref_name: String,
    pub base_ref_name: String,
    pub additions: u32,
    pub deletions: u32,
    pub commits: Option<CommitConnection>,
    pub reviews: Option<ReviewConnection>,
    pub comments: Option<CommentConnection>,
    pub files: Option<FileConnection>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewConnection {
    pub nodes: Vec<Option<ReviewNode>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewNode {
    pub author: Option<Actor>,
    pub state: Option<String>,
    pub body: Option<String>,
    /// Absent pour une relecture en attente, jamais soumise.
    pub submitted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CommentConnection {
    pub nodes: Vec<Option<CommentNode>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentNode {
    pub author: Option<Actor>,
    pub body: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct FileConnection {
    pub nodes: Vec<Option<FileNode>>,
}

#[derive(Debug, Deserialize)]
pub struct FileNode {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
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

impl PullRequestDetail {
    /// Assemble la vue détail autour du résumé déjà connu : la requête de
    /// détail ne renvoie aucun des champs de la liste.
    pub fn to_detail(&self, summary: PrSummary) -> PrDetail {
        let contextes = self
            .commits
            .as_ref()
            .and_then(|connexion| connexion.nodes.first())
            .and_then(|noeud| noeud.commit.status_check_rollup.as_ref())
            .and_then(|rollup| rollup.contexts.as_ref());

        PrDetail {
            summary,
            node_id: self.id.clone(),
            body: self.body.clone().unwrap_or_default(),
            head_ref: self.head_ref_name.clone(),
            base_ref: self.base_ref_name.clone(),
            checks: contextes
                .map(|connexion| {
                    connexion
                        .nodes
                        .iter()
                        .flatten()
                        .filter_map(ContextNode::to_check_run)
                        .collect()
                })
                .unwrap_or_default(),
            reviews: self
                .reviews
                .as_ref()
                .map(|connexion| {
                    connexion
                        .nodes
                        .iter()
                        .flatten()
                        .filter_map(ReviewNode::to_review)
                        .collect()
                })
                .unwrap_or_default(),
            comments: self
                .comments
                .as_ref()
                .map(|connexion| {
                    connexion
                        .nodes
                        .iter()
                        .flatten()
                        .filter_map(CommentNode::to_comment)
                        .collect()
                })
                .unwrap_or_default(),
            files: self
                .files
                .as_ref()
                .map(|connexion| {
                    connexion
                        .nodes
                        .iter()
                        .flatten()
                        .map(|fichier| ChangedFile {
                            path: fichier.path.clone(),
                            additions: fichier.additions,
                            deletions: fichier.deletions,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            additions: self.additions,
            deletions: self.deletions,
        }
    }
}

impl ContextNode {
    /// Les deux formes donnent la même entrée du modèle. Un nœud d'aucune des
    /// deux — GitHub peut ajouter un type à l'union — est ignoré.
    pub fn to_check_run(&self) -> Option<CheckRun> {
        if let Some(name) = &self.name {
            return Some(CheckRun {
                name: name.clone(),
                state: checks_from_check_run(self.status.as_deref(), self.conclusion.as_deref()),
                url: self.details_url.clone(),
            });
        }
        let context = self.context.as_ref()?;
        Some(CheckRun {
            name: context.clone(),
            state: checks_from_rollup(self.state.as_deref()),
            url: self.target_url.clone(),
        })
    }
}

impl ReviewNode {
    /// `None` pour une relecture en attente : elle n'a pas été soumise, donc
    /// elle n'a rien à dire à l'écran.
    pub fn to_review(&self) -> Option<Review> {
        Some(Review {
            author: login_ou_inconnu(self.author.as_ref()),
            state: review_from_review_state(self.state.as_deref()),
            body: self.body.clone().unwrap_or_default(),
            submitted_at: self.submitted_at?,
        })
    }
}

impl CommentNode {
    pub fn to_comment(&self) -> Option<Comment> {
        Some(Comment {
            author: login_ou_inconnu(self.author.as_ref()),
            body: self.body.clone().unwrap_or_default(),
            created_at: self.created_at?,
        })
    }
}

/// Auteur d'une relecture ou d'un commentaire, « inconnu » si le compte a été
/// supprimé.
fn login_ou_inconnu(auteur: Option<&Actor>) -> String {
    auteur
        .map(|auteur| auteur.login.clone())
        .unwrap_or_else(|| AUTEUR_INCONNU.to_string())
}

/// Un `CheckRun` en cours n'a pas encore de conclusion : son état vient de
/// `status`. Terminé, il vient de `conclusion`. Une conclusion neutre ou
/// sautée ne dit rien de la santé du code, donc `None` plutôt qu'un verdict.
fn checks_from_check_run(status: Option<&str>, conclusion: Option<&str>) -> ChecksState {
    if status != Some("COMPLETED") {
        return ChecksState::Pending;
    }
    match conclusion {
        Some("SUCCESS") => ChecksState::Success,
        Some("NEUTRAL") | Some("SKIPPED") => ChecksState::None,
        Some(_) => ChecksState::Failure,
        None => ChecksState::None,
    }
}

/// État d'une relecture individuelle. `COMMENTED`, `DISMISSED` et `PENDING`
/// n'ont pas d'équivalent : ils ne demandent ni ne donnent d'accord.
fn review_from_review_state(state: Option<&str>) -> ReviewState {
    match state {
        Some("APPROVED") => ReviewState::Approved,
        Some("CHANGES_REQUESTED") => ReviewState::ChangesRequested,
        _ => ReviewState::None,
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

    use crate::model::{ChangedFile, CheckRun, PrDetail};

    #[derive(Deserialize)]
    struct EnveloppeDetail {
        data: DetailData,
    }

    const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");

    /// Résumé quelconque : la requête de détail n'en renvoie aucun champ, le
    /// détail est assemblé autour de celui que la liste a déjà donné.
    fn resume() -> crate::model::PrSummary {
        page().pull_requests[0].clone()
    }

    fn detail() -> PrDetail {
        serde_json::from_str::<EnveloppeDetail>(DETAIL)
            .expect("la réponse enregistrée doit se lire")
            .data
            .repository
            .expect("dépôt présent")
            .pull_request
            .expect("pull request présente")
            .to_detail(resume())
    }

    #[test]
    fn le_detail_reprend_les_champs_de_la_pull_request() {
        let detail = detail();
        assert_eq!(detail.node_id, "PR_kwDOABCD12345");
        assert_eq!(
            detail.body,
            "Ajoute la fenêtre de fusion et ses raccourcis."
        );
        assert_eq!(detail.head_ref, "feat/fusion");
        assert_eq!(detail.base_ref, "develop");
        assert_eq!(detail.additions, 214);
        assert_eq!(detail.deletions, 37);
        assert_eq!(detail.summary, resume());
    }

    #[test]
    fn les_deux_formes_de_verification_donnent_des_entrees_equivalentes() {
        assert_eq!(
            detail().checks,
            vec![
                CheckRun {
                    name: "build".to_string(),
                    state: ChecksState::Success,
                    url: Some("https://github.com/moi/owl/actions/runs/1".to_string()),
                },
                CheckRun {
                    name: "clippy".to_string(),
                    state: ChecksState::Pending,
                    url: Some("https://github.com/moi/owl/actions/runs/2".to_string()),
                },
                CheckRun {
                    name: "documentation".to_string(),
                    state: ChecksState::None,
                    url: None,
                },
                CheckRun {
                    name: "ci/ancien-service".to_string(),
                    state: ChecksState::Failure,
                    url: Some("https://ancien.example/build/9".to_string()),
                },
                CheckRun {
                    name: "ci/attente".to_string(),
                    state: ChecksState::Pending,
                    url: None,
                },
            ],
            "un nœud d'aucune des deux formes est ignoré"
        );
    }

    #[test]
    fn les_relectures_sont_traduites_et_celles_en_attente_ignorees() {
        let detail = detail();
        assert_eq!(
            detail.reviews.len(),
            2,
            "la relecture en attente est ignorée"
        );
        assert_eq!(detail.reviews[0].author, "camille");
        assert_eq!(detail.reviews[0].state, ReviewState::Approved);
        assert_eq!(detail.reviews[0].body, "Bon pour moi.");
        assert_eq!(
            detail.reviews[0].submitted_at.to_rfc3339(),
            "2026-08-30T08:00:00+00:00"
        );
        assert_eq!(detail.reviews[1].author, "inconnu");
        assert_eq!(detail.reviews[1].state, ReviewState::ChangesRequested);
    }

    #[test]
    fn les_commentaires_et_les_fichiers_sont_traduits() {
        let detail = detail();
        assert_eq!(detail.comments.len(), 1);
        assert_eq!(detail.comments[0].author, "moi");
        assert_eq!(detail.comments[0].body, "Je rebase et je fusionne.");
        assert_eq!(
            detail.comments[0].created_at.to_rfc3339(),
            "2026-08-30T08:15:00+00:00"
        );
        assert_eq!(
            detail.files,
            vec![
                ChangedFile {
                    path: "src/ui/merge.rs".to_string(),
                    additions: 180,
                    deletions: 2,
                },
                ChangedFile {
                    path: "src/app.rs".to_string(),
                    additions: 34,
                    deletions: 35,
                },
            ]
        );
    }
}
