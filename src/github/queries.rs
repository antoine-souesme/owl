//! Requêtes GraphQL et mutation de fusion.
//!
//! Les documents sont recopiés de `docs/specs/01-modele-et-donnees.md` sans
//! reformulation : la spec fait foi, et une différence se lit d'un coup d'œil.

// `DETAIL` est appelée par la tâche suivante, `MERGE` par la spec 04.
#![allow(dead_code)]

/// Liste des pull requests. `$q` est la chaîne de recherche, `$n` le nombre
/// maximal de résultats.
///
/// Le fragment `... on PullRequest` est indispensable : `search` de type
/// `ISSUE` peut aussi renvoyer des issues.
pub const LIST: &str = r#"query List($q: String!, $n: Int!) {
  search(query: $q, type: ISSUE, first: $n) {
    nodes {
      ... on PullRequest {
        number
        title
        url
        isDraft
        mergeable
        reviewDecision
        updatedAt
        author { login }
        repository {
          nameWithOwner
          squashMergeAllowed
          mergeCommitAllowed
          rebaseMergeAllowed
          deleteBranchOnMerge
        }
        commits(last: 1) {
          nodes { commit { statusCheckRollup { state } } }
        }
      }
    }
  }
  rateLimit { remaining resetAt }
}"#;

/// Détail d'une seule pull request. Les listes sont volontairement bornées :
/// vingt relectures, vingt commentaires, cent fichiers, aucune pagination.
pub const DETAIL: &str = r#"query Detail($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      id
      body
      headRefName
      baseRefName
      additions
      deletions
      commits(last: 1) {
        nodes { commit { statusCheckRollup { contexts(first: 50) { nodes {
          ... on CheckRun { name conclusion status detailsUrl }
          ... on StatusContext { context state targetUrl }
        } } } } }
      }
      reviews(last: 20) { nodes { author { login } state body submittedAt } }
      comments(last: 20) { nodes { author { login } body createdAt } }
      files(first: 100) { nodes { path additions deletions } }
    }
  }
}"#;

/// Fusion. `owl` ne demande jamais la suppression de la branche : elle suit
/// le réglage `deleteBranchOnMerge` du dépôt, appliqué par GitHub lui-même.
/// L'appel est déclenché par `docs/specs/04-fusion.md`.
pub const MERGE: &str = r#"mutation Merge($id: ID!, $method: PullRequestMergeMethod!) {
  mergePullRequest(input: { pullRequestId: $id, mergeMethod: $method }) {
    pullRequest { number state }
  }
}"#;
