# 01 — Modèle métier et accès aux données

## Objet

Décrit les types métier de `owl` et la façon dont ils sont remplis depuis l'API
GraphQL de GitHub.

## Principe

Les réponses de l'API sont lues dans des types bruts (`github::dto`), puis traduites
en types métier (`model`). Le reste du programme ne voit que les types métier. Cette
frontière permet de changer de requête, ou même d'API, sans toucher à l'écran.

## Types métier

```rust
/// Identité d'une pull request, stable et utilisable comme clé.
struct PrKey { repo: String, number: u32 }   // repo au format "org/dépôt"

/// Ce qu'il faut pour dessiner une ligne de liste.
struct PrSummary {
    key: PrKey,
    title: String,
    author: String,
    url: String,
    is_draft: bool,
    checks: ChecksState,
    review: ReviewState,
    mergeable: MergeableState,
    updated_at: DateTime<Utc>,
    repo_rules: RepoMergeRules,
}

enum ChecksState { Success, Failure, Pending, None }

enum ReviewState { Approved, ChangesRequested, ReviewRequired, None }

enum MergeableState { Mergeable, Conflicting, Unknown }

/// Méthodes de fusion autorisées par le dépôt.
struct RepoMergeRules {
    squash: bool,
    merge: bool,
    rebase: bool,
    delete_branch_on_merge: bool,
}

/// Ce qu'il faut en plus pour dessiner la vue détail.
struct PrDetail {
    summary: PrSummary,
    node_id: String,          // identifiant GraphQL, nécessaire à la fusion
    body: String,
    head_ref: String,
    base_ref: String,
    checks: Vec<CheckRun>,
    reviews: Vec<Review>,
    comments: Vec<Comment>,
    files: Vec<ChangedFile>,
    additions: u32,
    deletions: u32,
}

struct CheckRun { name: String, state: ChecksState, url: Option<String> }
struct Review { author: String, state: ReviewState, body: String, submitted_at: DateTime<Utc> }
struct Comment { author: String, body: String, created_at: DateTime<Utc> }
struct ChangedFile { path: String, additions: u32, deletions: u32 }
```

`RepoMergeRules` est porté par `PrSummary` et non par une structure de dépôt séparée :
l'information arrive dans la même requête que la liste, et la fenêtre de fusion en a
besoin sans appel supplémentaire.

## Traduction des états

`ChecksState` vient du `statusCheckRollup` du dernier commit :

| Valeur GitHub | `ChecksState` |
|---|---|
| `SUCCESS` | `Success` |
| `FAILURE`, `ERROR` | `Failure` |
| `PENDING`, `EXPECTED` | `Pending` |
| absent (aucune CI) | `None` |

`ReviewState` vient de `reviewDecision` :

| Valeur GitHub | `ReviewState` |
|---|---|
| `APPROVED` | `Approved` |
| `CHANGES_REQUESTED` | `ChangesRequested` |
| `REVIEW_REQUIRED` | `ReviewRequired` |
| `null` | `None` |

`MergeableState` vient de `mergeable`. GitHub calcule ce champ de façon paresseuse et
renvoie `UNKNOWN` le temps du calcul ; `owl` traite `UNKNOWN` comme « on ne sait pas
encore » et non comme un blocage.

## Requête de liste

Une seule requête, du type `search`, portant la chaîne construite par le module
`filter` (voir `02-filtres.md`).

```graphql
query List($q: String!, $n: Int!) {
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
}
```

Le fragment `... on PullRequest` est indispensable : `search` de type `ISSUE` peut
aussi renvoyer des issues. Un nœud qui n'est pas une pull request est ignoré
silencieusement.

`rateLimit` est lu à chaque requête et conservé dans l'état, pour le traitement des
limites d'appels décrit en `05-erreurs-et-tests.md`.

## Requête de détail

Lancée à l'ouverture de la vue détail, pour une seule PR.

```graphql
query Detail($owner: String!, $name: String!, $number: Int!) {
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
}
```

Les vérifications arrivent sous deux formes, `CheckRun` (GitHub Actions et
équivalents) et `StatusContext` (anciens statuts d'API). Les deux sont traduites vers
`CheckRun` du modèle.

Les listes sont volontairement bornées : les vingt dernières relectures, les vingt
derniers commentaires, les cent premiers fichiers. Il n'y a pas de pagination dans la
vue détail. Quand une liste est tronquée, l'écran l'indique par une ligne
« … et N de plus ».

## Mutation de fusion

```graphql
mutation Merge($id: ID!, $method: PullRequestMergeMethod!) {
  mergePullRequest(input: { pullRequestId: $id, mergeMethod: $method }) {
    pullRequest { number state }
  }
}
```

La fusion a besoin de l'identifiant GraphQL de la PR, absent de la requête de liste.
Il est donc récupéré juste avant la fusion si la vue détail n'a pas déjà été ouverte.
Ce point est détaillé en `04-fusion.md`.

La suppression de la branche après fusion n'est pas demandée par `owl` : elle suit le
réglage `deleteBranchOnMerge` du dépôt, appliqué par GitHub lui-même.

## Client GraphQL

Un seul point d'entrée HTTP : `POST https://api.github.com/graphql`, avec les
en-têtes `Authorization: Bearer <jeton>` et `User-Agent: owl/<version>`.

Le client distingue quatre issues, et c'est cette distinction qui pilote le
traitement des erreurs :

- succès, avec des données ;
- réponse HTTP 200 contenant un tableau `errors` — erreur applicative ;
- réponse HTTP 401 ou 403 — jeton invalide ou droits insuffisants ;
- limite d'appels atteinte, reconnue à la réponse 403 accompagnée d'un en-tête de
  réinitialisation, ou à un `rateLimit.remaining` nul.

## Critères de réussite

- Une réponse de liste enregistrée sur disque se traduit en `Vec<PrSummary>` exact,
  états compris.
- Un nœud de type issue mélangé dans la réponse est ignoré sans faire échouer la
  traduction.
- Une PR sans aucune CI donne `ChecksState::None`, distinct de `Pending`.
- Les deux formes de vérification, `CheckRun` et `StatusContext`, produisent des
  entrées équivalentes dans `PrDetail::checks`.
