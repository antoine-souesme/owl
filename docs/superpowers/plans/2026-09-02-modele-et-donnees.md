# Modèle métier et accès aux données — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remplacer le bouchon réseau des fondations par le vrai accès aux données : les types métier de la spec, un client GraphQL qui distingue ses quatre issues, et la traduction des réponses de GitHub vers ces types.

**Architecture:** Une frontière nette. `github::dto` contient des types `serde` qui collent à la forme exacte des réponses de GitHub, avec des champs optionnels partout où GitHub peut ne rien renvoyer. Ces types savent se traduire en types `model`, et c'est le seul endroit qui connaît le vocabulaire de GitHub (`SUCCESS`, `CHANGES_REQUESTED`, `nameWithOwner`). `github::mod` tient un `Client` unique : il porte le jeton dans ses en-têtes par défaut, envoie les requêtes de `github::queries`, et classe chaque réponse en succès, erreur applicative, jeton refusé, droits insuffisants ou limite d'appels. `app` reçoit le résultat traduit, ne fait toujours aucun appel réseau, et `ui` continue de ne rien décider.

**Tech Stack:** Rust édition 2021, `reqwest` + `rustls` (HTTP), `serde` / `serde_json` (réponses), `chrono` avec la fonction `serde` (dates), `thiserror` (erreurs du domaine), `tokio` (exécution et, en test, serveur local).

**Spec:** `docs/specs/01-modele-et-donnees.md` (contexte transverse : `docs/specs/00-fondations.md` pour les règles de dépendance, `docs/specs/05-erreurs-et-tests.md` pour les messages d'erreur et la stratégie de test, `docs/specs/02-filtres.md` pour la chaîne de recherche encore absente)

Aucun lien de design n'a été fourni pour cette spec, et elle n'ajoute aucun écran : elle remplit la liste déjà dessinée par les fondations.

## Conditions d'exécution

- Exécution en **subagent-driven development** : un sous-agent par tâche, revue entre chaque.
- Le registre `sdd` du dépôt est vide au départ (`.superpowers/sdd/` ne contient que son `.gitignore`) : repartir de zéro, aucun état antérieur à reprendre.
- Branche de travail : `feat/modele-et-donnees`, créée depuis `develop` à la première étape de la tâche 1. Jamais de travail direct sur `develop`, jamais de pull request vers `main`.
- Ce fichier de plan n'est pas encore suivi par git : l'ajouter au premier commit de la branche.
- Aucune question à poser avant de commencer : tout ce qui est nécessaire est dans ce plan et dans la spec. Une question n'est légitime que devant un blocage réel qui empêche la suite — un outil absent, un service indisponible.
- Une décision **mise de côté** se consigne dans `docs/suivi/DETTE.md`, au format déjà en place dans ce fichier, et uniquement si elle est critique pour la suite. Pas les décisions prises, pas les idées d'amélioration, aucune sur-conception.
- À la fin : pull request vers `develop`, et un rapport qui ne raconte pas le travail fait. Voir la tâche 5, étape 7.

## Global Constraints

- Rust, édition 2021. Binaire unique nommé `owl`, aucune sous-commande.
- Aucune nouvelle dépendance de production. Seul ajout autorisé : la fonction `serde` de `chrono`, nécessaire pour lire les dates de GitHub, et les fonctions `net` / `io-util` de `tokio` en dépendance de développement, pour le serveur local des tests.
- Arborescence des modules inchangée, exactement celle de `00-fondations.md` : `main.rs`, `config.rs`, `token.rs`, `github/{mod,queries,dto}.rs`, `model.rs`, `filter.rs`, `app.rs`, `ui/{mod,list,detail,merge}.rs`. **Aucun nouveau fichier source.** Le type d'erreur et le client vivent dans `github/mod.rs`.
- Dépendances à sens unique : `model` ne dépend que de la bibliothèque standard, de `serde` et de `chrono` ; `github` dépend de `model` ; `github` ne connaît ni `app` ni `ui` ; `app` ne fait aucun appel réseau ; une fonction de dessin ne modifie jamais l'état et ne décide de rien.
- Le jeton n'est jamais écrit dans un fichier, ni journalisé, ni affiché. Son en-tête est marqué sensible et aucune erreur ne transporte de message de `reqwest` susceptible de contenir l'URL ou les en-têtes.
- Un seul point d'entrée HTTP : `POST https://api.github.com/graphql`, en-têtes `Authorization: Bearer <jeton>` et `User-Agent: owl/<version>`.
- Les messages d'erreur de GitHub sont affichés tels quels, sans reformulation.
- Traduction des états, au mot près (`01-modele-et-donnees.md`) :
  - `statusCheckRollup.state` : `SUCCESS` → `Success` ; `FAILURE`, `ERROR` → `Failure` ; `PENDING`, `EXPECTED` → `Pending` ; absent → `None`.
  - `reviewDecision` : `APPROVED` → `Approved` ; `CHANGES_REQUESTED` → `ChangesRequested` ; `REVIEW_REQUIRED` → `ReviewRequired` ; `null` → `None`.
  - `mergeable` : `MERGEABLE` → `Mergeable` ; `CONFLICTING` → `Conflicting` ; `UNKNOWN` → `Unknown`, traité comme « on ne sait pas encore » et non comme un blocage.
- Un nœud de `search` qui n'est pas une pull request est ignoré silencieusement.
- Les deux formes de vérification, `CheckRun` et `StatusContext`, produisent des entrées équivalentes dans `PrDetail::checks`.
- Les listes de la vue détail restent bornées telles quelles : 20 relectures, 20 commentaires, 100 fichiers, aucune pagination.
- La suppression de branche après fusion n'est jamais demandée par `owl`.
- Le projet est en français : messages affichés, commentaires, messages de commit. Les identifiants du code restent en anglais.
- `cargo build`, `cargo test`, `cargo clippy -- -D warnings` et `cargo fmt --check` doivent passer à la fin de chaque tâche. Aucune n'est optionnelle.
- Branche de travail : `feat/modele-et-donnees`, créée depuis `develop` à la première étape de la tâche 1. Ne jamais travailler directement sur `develop`. Aucune pull request vers `main`.

## Décisions mises de côté

Toute décision **reportée** pendant l'exécution se consigne dans `docs/suivi/DETTE.md`, au format déjà en place dans ce fichier. Uniquement ce qui est reporté et critique pour la suite — pas les décisions prises, pas les idées d'amélioration, pas de sur-conception.

## Décisions prises en écrivant ce plan

Elles sortent du texte de la spec et sont donc reportées dans `docs/specs/01-modele-et-donnees.md` par les tâches concernées, comme l'exige l'ordre de vérité du `CLAUDE.md`.

1. **`app` connaît `GithubError`.** La spec impose de remplacer le `String` d'erreur par un type `thiserror` et dit que `app::Event::Data` change en conséquence. `app` importe donc `github::GithubError` pour l'afficher. Les règles de dépendance interdisent à `github` de connaître `app`, pas l'inverse, et `app` ne gagne aucun appel réseau au passage.
2. **Le solde d'appels voyage avec la liste.** La spec demande que `rateLimit` soit lu à chaque requête et conservé dans l'état. Le client renvoie donc un `ListPage { pull_requests, rate_limit }` et `app` garde le solde sans rien en faire : la suspension du minuteur appartient à `05-erreurs-et-tests.md`.
3. **Un `remaining` nul n'est pas une erreur.** Quand la réponse est réussie mais que le solde est à zéro, les données sont bonnes : elles sont rendues, et le solde nul est transmis. `GithubError::RateLimited` est réservé au refus de GitHub, reconnu à une réponse 403 accompagnée d'un en-tête de réinitialisation.
4. **La chaîne de recherche est une simple jointure.** `filter::build_query` n'existe pas encore ; `github` joint les filtres avec une espace, sans ajouter `is:pr`, pour ne pas dupliquer une règle qui appartient à `02-filtres.md`.
5. **La mutation de fusion est posée en constante, pas en fonction.** La spec renvoie explicitement son déclenchement à `04-fusion.md`, et la fonction aurait besoin du type de méthode de fusion qui vit dans `config`, un module dont `github` ne doit pas dépendre.
6. **Cas limites de traduction absents de la spec** : auteur `null` (compte supprimé) → `model::AUTEUR_INCONNU`, soit « inconnu » ; relecture sans `submittedAt` (relecture en attente) → ignorée ; `CheckRun` dont `status` n'est pas `COMPLETED` → `Pending`, conclusion `NEUTRAL` ou `SKIPPED` → `None`, toute autre conclusion → `Failure`.
7. **La ligne « … et N de plus » n'est pas réalisable avec ces requêtes** : elles ne demandent aucun `totalCount`. Le point part en dette, à traiter par la spec 03 qui possède cet affichage.

## Structure des fichiers

| Fichier | Responsabilité | Tâche |
|---|---|---|
| `Cargo.toml` | Ajout de la fonction `serde` de `chrono` et des fonctions `net` / `io-util` de `tokio` en développement | 1 puis 3 |
| `src/model.rs` | Tous les types métier de la spec, plus `ListPage` et `RateLimit`. Aucune connaissance du vocabulaire de GitHub | 1 |
| `src/github/queries.rs` | Les trois documents GraphQL, en constantes | 3 |
| `src/github/dto.rs` | Types de réponse brute et traduction vers `model` — liste en tâche 2, détail en tâche 4 | 2 puis 4 |
| `src/github/mod.rs` | `GithubError`, `Client`, les quatre issues d'une réponse, `fetch_pull_requests`, `fetch_detail` | 3 puis 4 |
| `src/app.rs` | `Event::Data` porte `Result<ListPage, GithubError>` ; le solde d'appels est conservé | 1 puis 3 |
| `src/main.rs` | Construit un `Client` unique et le partage entre les tâches de requête | 3 |
| `src/ui/list.rs` | Lit les nouveaux champs pour dessiner une ligne | 1 |
| `tests/fixtures/list.json` | Réponse de liste enregistrée, formes surprenantes comprises | 2 |
| `tests/fixtures/detail.json` | Réponse de détail enregistrée, les deux formes de vérification comprises | 4 |
| `docs/specs/01-modele-et-donnees.md` | Reçoit les décisions prises en cours de route | 2, 3, 4 |
| `docs/suivi/DETTE.md` | Reçoit les points différés | 4 |

Les tests de traduction sont des tests unitaires dans `src/github/dto.rs` qui lisent les fichiers de `tests/fixtures/` par `include_str!`. C'est imposé par la nature du projet : `owl` est un binaire, ses modules ne sont pas accessibles depuis un test d'intégration.

---

### Task 1: Types métier de la spec

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/model.rs` (remplacement complet)
- Modify: `src/github/mod.rs:6-21`
- Modify: `src/app.rs`
- Modify: `src/ui/list.rs:17-22`

**Interfaces:**
- Consumes: `crate::config::Config` (inchangé).
- Produces, tous dans `model` :
  - `PrKey { repo: String, number: u32 }`, avec `PrKey::owner(&self) -> &str` et `PrKey::name(&self) -> &str` qui découpent `"org/dépôt"`.
  - `PrSummary { key: PrKey, title: String, author: String, url: String, is_draft: bool, checks: ChecksState, review: ReviewState, mergeable: MergeableState, updated_at: DateTime<Utc>, repo_rules: RepoMergeRules }`.
  - `ChecksState { Success, Failure, Pending, None }`, `ReviewState { Approved, ChangesRequested, ReviewRequired, None }`, `MergeableState { Mergeable, Conflicting, Unknown }`.
  - `RepoMergeRules { squash: bool, merge: bool, rebase: bool, delete_branch_on_merge: bool }`.
  - `RateLimit { remaining: u32, reset_at: DateTime<Utc> }`.
  - `ListPage { pull_requests: Vec<PrSummary>, rate_limit: Option<RateLimit> }`.
  - `PrDetail`, `CheckRun`, `Review`, `Comment`, `ChangedFile`, exactement les champs de la spec.
  - `AUTEUR_INCONNU: &str = "inconnu"`.
- Le type `PullRequest` des fondations disparaît. `app::App::items` devient `Vec<PrSummary>`.

- [ ] **Step 1: Créer la branche de travail**

```bash
git switch develop
git pull --ff-only
git switch -c feat/modele-et-donnees
```

- [ ] **Step 2: Activer la lecture des dates par `serde`**

Dans `Cargo.toml`, remplacer la ligne de `chrono` par :

```toml
chrono = { version = "0.4", features = ["serde"] }
```

Sans cette fonction, `DateTime<Utc>` ne se déserialise pas et la tâche 2 ne compilerait pas. Aucune autre ligne de `Cargo.toml` ne change.

- [ ] **Step 3: Écrire le test qui échoue**

Remplacer entièrement `src/model.rs` par le seul test, pour le voir échouer avant d'écrire les types :

```rust
//! Types métier. Ne dépend ni du réseau ni du terminal.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_cle_separe_le_proprietaire_et_le_depot() {
        let cle = PrKey {
            repo: "moi/owl".to_string(),
            number: 42,
        };
        assert_eq!(cle.owner(), "moi");
        assert_eq!(cle.name(), "owl");
    }

    #[test]
    fn une_cle_sans_barre_oblique_ne_panique_pas() {
        let cle = PrKey {
            repo: "owl".to_string(),
            number: 1,
        };
        assert_eq!(cle.owner(), "owl");
        assert_eq!(cle.name(), "");
    }
}
```

- [ ] **Step 4: Lancer le test pour vérifier qu'il échoue**

Run: `cargo test --lib model 2>&1 | head -20`
Expected: FAIL à la compilation, `cannot find struct, variant or union type PrKey`.

- [ ] **Step 5: Écrire les types métier**

Insérer avant le module de tests, dans `src/model.rs` :

```rust
//! Types métier. Ne dépend ni du réseau ni du terminal.
//!
//! Ces types sont ceux de `docs/specs/01-modele-et-donnees.md`. Ils ne
//! connaissent pas le vocabulaire de GitHub : la traduction est faite par
//! `github::dto`, seul endroit qui voit passer un `SUCCESS` ou un
//! `nameWithOwner`.

// Les specs 03 et 04 consomment la vue détail et les règles de fusion. D'ici
// là, une partie de ces champs n'est lue que par les tests de traduction.
#![allow(dead_code)]

use chrono::{DateTime, Utc};

/// Auteur affiché quand GitHub n'en renvoie aucun : le compte a été supprimé.
pub const AUTEUR_INCONNU: &str = "inconnu";

/// Identité d'une pull request, stable et utilisable comme clé.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrKey {
    /// Dépôt au format `org/dépôt`.
    pub repo: String,
    pub number: u32,
}

impl PrKey {
    /// Propriétaire du dépôt, partie gauche de `org/dépôt`.
    pub fn owner(&self) -> &str {
        self.repo.split('/').next().unwrap_or(&self.repo)
    }

    /// Nom du dépôt, partie droite de `org/dépôt`.
    pub fn name(&self) -> &str {
        self.repo.split('/').nth(1).unwrap_or("")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksState {
    Success,
    Failure,
    Pending,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    ReviewRequired,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeableState {
    Mergeable,
    Conflicting,
    /// GitHub calcule ce champ paresseusement : « on ne sait pas encore »,
    /// et non un blocage.
    Unknown,
}

/// Méthodes de fusion autorisées par le dépôt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoMergeRules {
    pub squash: bool,
    pub merge: bool,
    pub rebase: bool,
    pub delete_branch_on_merge: bool,
}

/// Ce qu'il faut pour dessiner une ligne de liste.
///
/// `repo_rules` est porté ici et non par une structure de dépôt séparée :
/// l'information arrive dans la même requête que la liste, et la fenêtre de
/// fusion en a besoin sans appel supplémentaire.
#[derive(Debug, Clone, PartialEq)]
pub struct PrSummary {
    pub key: PrKey,
    pub title: String,
    pub author: String,
    pub url: String,
    pub is_draft: bool,
    pub checks: ChecksState,
    pub review: ReviewState,
    pub mergeable: MergeableState,
    pub updated_at: DateTime<Utc>,
    pub repo_rules: RepoMergeRules,
}

/// Solde d'appels restant, lu à chaque requête réussie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub remaining: u32,
    pub reset_at: DateTime<Utc>,
}

/// Résultat d'une requête de liste : les pull requests et le solde d'appels
/// lu au passage. Le solde voyage avec les données parce que la spec demande
/// qu'il soit conservé dans l'état à chaque requête.
#[derive(Debug, Clone, PartialEq)]
pub struct ListPage {
    pub pull_requests: Vec<PrSummary>,
    pub rate_limit: Option<RateLimit>,
}

/// Ce qu'il faut en plus pour dessiner la vue détail.
#[derive(Debug, Clone, PartialEq)]
pub struct PrDetail {
    pub summary: PrSummary,
    /// Identifiant GraphQL, nécessaire à la fusion.
    pub node_id: String,
    pub body: String,
    pub head_ref: String,
    pub base_ref: String,
    pub checks: Vec<CheckRun>,
    pub reviews: Vec<Review>,
    pub comments: Vec<Comment>,
    pub files: Vec<ChangedFile>,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckRun {
    pub name: String,
    pub state: ChecksState,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub author: String,
    pub state: ReviewState,
    pub body: String,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangedFile {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
}
```

- [ ] **Step 6: Lancer le test pour vérifier qu'il passe**

Run: `cargo test --lib model 2>&1 | tail -20`
Expected: les deux tests de `model` passent. Le reste du projet ne compile plus : `PullRequest` a disparu, c'est attendu et réparé aux étapes suivantes.

- [ ] **Step 7: Adapter le bouchon de `github`**

Dans `src/github/mod.rs`, remplacer l'import et la signature :

```rust
use crate::model::{ListPage, PrSummary};
```

et le corps de la fonction :

```rust
pub async fn fetch_pull_requests(
    _token: &str,
    _filters: &[String],
    _page_size: u16,
) -> Result<ListPage, String> {
    Ok(ListPage {
        pull_requests: Vec::<PrSummary>::new(),
        rate_limit: None,
    })
}
```

Le bouchon reste un bouchon : c'est la tâche 3 qui le remplace par le vrai client. Cette étape ne fait que suivre le changement de types.

- [ ] **Step 8: Adapter `app` aux nouveaux types**

Dans `src/app.rs`, remplacer l'import `use crate::model::PullRequest;` par :

```rust
use crate::model::{ListPage, PrSummary};
```

Dans `Event::Data`, changer le type du résultat :

```rust
    /// Résultat d'une demande réseau.
    Data {
        generation: Generation,
        result: Result<ListPage, String>,
    },
```

Dans `App`, changer le type de `items` et ajouter le solde d'appels :

```rust
pub struct App {
    pub items: Vec<PrSummary>,
    /// Solde d'appels rapporté par la dernière requête réussie. Conservé ici
    /// parce que la spec 01 le demande ; la suspension du rafraîchissement
    /// qu'il déclenche appartient à `05-erreurs-et-tests.md`.
    #[allow(dead_code)]
    pub rate_limit: Option<crate::model::RateLimit>,
    ...
}
```

Initialiser `rate_limit: None` dans `App::new`, et dans le bras `Ok` de `Event::Data` :

```rust
                    Ok(page) => {
                        self.items = page.pull_requests;
                        self.rate_limit = page.rate_limit;
                        self.last_refresh = Some(Local::now());
                        self.status = self.liste_resumee();
                    }
```

- [ ] **Step 9: Adapter les tests de `app`**

Dans le module de tests de `src/app.rs`, remplacer l'aide `pr` et ajouter une aide de page. Les tests existants passent alors `Ok(page(vec![pr(1)]))` au lieu de `Ok(vec![pr(1)])`, et comparent `app.items` à `vec![pr(1)]` sans autre changement.

```rust
    use crate::model::{
        ChecksState, ListPage, MergeableState, PrKey, PrSummary, RepoMergeRules, ReviewState,
    };

    fn pr(numero: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: "moi/depot".to_string(),
                number: numero,
            },
            title: format!("Titre {numero}"),
            author: "moi".to_string(),
            url: format!("https://github.com/moi/depot/pull/{numero}"),
            is_draft: false,
            checks: ChecksState::Success,
            review: ReviewState::Approved,
            mergeable: MergeableState::Mergeable,
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            },
        }
    }

    /// Réponse de liste sans solde d'appels, suffisante partout où seul le
    /// contenu de la liste compte.
    fn page(pull_requests: Vec<PrSummary>) -> ListPage {
        ListPage {
            pull_requests,
            rate_limit: None,
        }
    }
```

Remplacer mécaniquement, dans tous les tests du module, `result: Ok(vec![` par `result: Ok(page(vec![` et fermer la parenthèse ajoutée. Le cas vide devient `result: Ok(page(vec![]))`.

- [ ] **Step 10: Adapter le dessin de la liste**

Dans `src/ui/list.rs`, la composition du texte lit les nouveaux champs :

```rust
        .map(|pr| {
            ListItem::new(format!(
                "{}#{}  {}",
                pr.key.repo, pr.key.number, pr.title
            ))
        })
```

Ne rien ajouter d'autre ici : cette composition est déjà inscrite dans `docs/suivi/DETTE.md` et la spec 03 la remplacera. L'aggraver en y ajoutant des pictogrammes serait une décision d'affichage prise au mauvais endroit.

- [ ] **Step 11: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe. Aucun test n'a été supprimé au passage : `cargo test` doit rapporter au moins autant de tests qu'avant.

- [ ] **Step 12: Commit**

```bash
git add docs/superpowers/plans/2026-09-02-modele-et-donnees.md
git add Cargo.toml Cargo.lock src/model.rs src/github/mod.rs src/app.rs src/ui/list.rs
git commit -m "Pose les types métier de la spec 01"
```

---

### Task 2: Lecture et traduction de la réponse de liste

**Files:**
- Create: `tests/fixtures/list.json`
- Modify: `src/github/dto.rs` (remplacement complet)
- Modify: `docs/specs/01-modele-et-donnees.md`

**Interfaces:**
- Consumes: tous les types de `model` produits par la tâche 1.
- Produces, dans `github::dto` :
  - `ListData { search: Search, rate_limit: Option<RateLimitDto> }` — déserialisable depuis le contenu du champ `data` d'une réponse de liste.
  - `ListData::to_list_page(&self) -> model::ListPage`.
  - `SearchNode::to_summary(&self) -> Option<model::PrSummary>` — `None` quand le nœud n'est pas une pull request.
  - `CommitConnection`, `CommitNode`, `Commit`, `Rollup`, `Actor`, `RepositoryDto`, `RateLimitDto` — réutilisés par la tâche 4.
  - `checks_from_rollup(state: Option<&str>) -> model::ChecksState` — réutilisée par la tâche 4 pour les `StatusContext`.

- [ ] **Step 1: Enregistrer la réponse de liste**

Créer `tests/fixtures/list.json`. Ce fichier est la référence du projet : il décrit ce que GitHub renvoie vraiment, formes surprenantes comprises. Six nœuds, dans cet ordre : une pull request complète, un nœud d'issue vide, un brouillon sans aucun commit et sans auteur, une pull request en conflit avec des vérifications en attente, une pull request en échec, et une pull request dont le dernier commit n'a pas de `statusCheckRollup`.

```json
{
  "data": {
    "search": {
      "nodes": [
        {
          "number": 42,
          "title": "Ajoute la fenêtre de fusion",
          "url": "https://github.com/moi/owl/pull/42",
          "isDraft": false,
          "mergeable": "MERGEABLE",
          "reviewDecision": "APPROVED",
          "updatedAt": "2026-08-30T09:12:44Z",
          "author": { "login": "moi" },
          "repository": {
            "nameWithOwner": "moi/owl",
            "squashMergeAllowed": true,
            "mergeCommitAllowed": false,
            "rebaseMergeAllowed": true,
            "deleteBranchOnMerge": true
          },
          "commits": {
            "nodes": [{ "commit": { "statusCheckRollup": { "state": "SUCCESS" } } }]
          }
        },
        {},
        {
          "number": 7,
          "title": "Brouillon sans intégration continue",
          "url": "https://github.com/moi/notes/pull/7",
          "isDraft": true,
          "mergeable": "UNKNOWN",
          "reviewDecision": null,
          "updatedAt": "2026-08-29T18:03:00Z",
          "author": null,
          "repository": {
            "nameWithOwner": "moi/notes",
            "squashMergeAllowed": false,
            "mergeCommitAllowed": true,
            "rebaseMergeAllowed": false,
            "deleteBranchOnMerge": false
          },
          "commits": { "nodes": [] }
        },
        {
          "number": 13,
          "title": "Corrige la troncature des titres",
          "url": "https://github.com/uneorg/site/pull/13",
          "isDraft": false,
          "mergeable": "CONFLICTING",
          "reviewDecision": "CHANGES_REQUESTED",
          "updatedAt": "2026-08-28T07:45:10Z",
          "author": { "login": "camille" },
          "repository": {
            "nameWithOwner": "uneorg/site",
            "squashMergeAllowed": true,
            "mergeCommitAllowed": true,
            "rebaseMergeAllowed": false,
            "deleteBranchOnMerge": false
          },
          "commits": {
            "nodes": [{ "commit": { "statusCheckRollup": { "state": "PENDING" } } }]
          }
        },
        {
          "number": 99,
          "title": "Migre vers ratatui 0.30",
          "url": "https://github.com/moi/owl/pull/99",
          "isDraft": false,
          "mergeable": "MERGEABLE",
          "reviewDecision": "REVIEW_REQUIRED",
          "updatedAt": "2026-08-27T22:00:00Z",
          "author": { "login": "moi" },
          "repository": {
            "nameWithOwner": "moi/owl",
            "squashMergeAllowed": true,
            "mergeCommitAllowed": false,
            "rebaseMergeAllowed": true,
            "deleteBranchOnMerge": true
          },
          "commits": {
            "nodes": [{ "commit": { "statusCheckRollup": { "state": "FAILURE" } } }]
          }
        },
        {
          "number": 3,
          "title": "Dépôt sans intégration continue configurée",
          "url": "https://github.com/moi/notes/pull/3",
          "isDraft": false,
          "mergeable": "MERGEABLE",
          "reviewDecision": null,
          "updatedAt": "2026-08-26T11:20:00Z",
          "author": { "login": "moi" },
          "repository": {
            "nameWithOwner": "moi/notes",
            "squashMergeAllowed": false,
            "mergeCommitAllowed": true,
            "rebaseMergeAllowed": false,
            "deleteBranchOnMerge": false
          },
          "commits": {
            "nodes": [{ "commit": { "statusCheckRollup": null } }]
          }
        }
      ]
    },
    "rateLimit": { "remaining": 4987, "resetAt": "2026-08-30T10:00:00Z" }
  }
}
```

- [ ] **Step 2: Écrire les tests qui échouent**

Remplacer entièrement `src/github/dto.rs` par le module d'en-tête et les tests, pour les voir échouer avant d'écrire la traduction :

```rust
//! Types de réponse brute de l'API, mappés vers `model`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ChecksState, ListPage, MergeableState, PrKey, RepoMergeRules, ReviewState,
    };
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
```

- [ ] **Step 3: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --lib github::dto 2>&1 | head -20`
Expected: FAIL à la compilation, `cannot find type ListData in this scope`.

- [ ] **Step 4: Écrire les types bruts et la traduction**

Insérer avant le module de tests, dans `src/github/dto.rs` :

```rust
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
```

- [ ] **Step 5: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test --lib github::dto 2>&1 | tail -20`
Expected: les sept tests passent.

- [ ] **Step 6: Reporter les décisions dans la spec**

Dans `docs/specs/01-modele-et-donnees.md`, ajouter à la fin de la section « Traduction des états » :

```markdown
Les cas que l'API laisse ouverts sont tranchés ainsi :

| Situation | Traduction |
|---|---|
| `author` à `null` (compte supprimé) | auteur affiché « inconnu » |
| Aucun commit, ou `statusCheckRollup` absent | `ChecksState::None` |
| Valeur d'état inconnue de la table | traitée comme une absence : `None`, ou `Unknown` pour `mergeable` |
| Nœud sans les champs d'une pull request | ignoré, sans erreur |
```

- [ ] **Step 7: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe.

- [ ] **Step 8: Commit**

```bash
git add tests/fixtures/list.json src/github/dto.rs docs/specs/01-modele-et-donnees.md
git commit -m "Traduit la réponse de liste en types métier"
```

---

### Task 3: Client GraphQL et branchement de la liste

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/github/queries.rs` (remplacement complet)
- Modify: `src/github/mod.rs` (remplacement complet)
- Modify: `src/app.rs`
- Modify: `src/main.rs:106-175`
- Modify: `docs/specs/01-modele-et-donnees.md`

**Interfaces:**
- Consumes: `dto::ListData` et `ListData::to_list_page` de la tâche 2.
- Produces :
  - `github::queries::LIST`, `github::queries::DETAIL`, `github::queries::MERGE` — les trois documents GraphQL de la spec, mot pour mot.
  - `github::GithubError` — `enum` `thiserror`. Variantes : `Api(String)`, `Unauthorized`, `Forbidden`, `RateLimited { reset_at: Option<DateTime<Utc>> }`, `Http(u16)`, `Malformed`, `Transport`, `NotFound`.
  - `github::Client` — `Client::new(token: &str) -> Result<Client, GithubError>`, `Client::with_endpoint(token: &str, endpoint: &str) -> Result<Client, GithubError>`, `Client::fetch_pull_requests(&self, filters: &[String], page_size: u16) -> Result<model::ListPage, GithubError>`.
  - `app::Event::Data` porte désormais `Result<model::ListPage, GithubError>`.
  - `app::Command::Fetch` est inchangée : `app` continue de n'émettre qu'une demande.
- La fonction libre `github::fetch_pull_requests` disparaît, remplacée par la méthode du client.

- [ ] **Step 1: Ouvrir un serveur local aux tests**

Dans `Cargo.toml`, remplacer la section des dépendances de développement par :

```toml
[dev-dependencies]
tempfile = "3"
tokio = { version = "1", features = ["net", "io-util", "rt-multi-thread", "macros"] }
```

`tokio` est déjà une dépendance de production ; ces deux fonctions supplémentaires ne servent qu'au serveur local des tests, comme prévu par `05-erreurs-et-tests.md`.

- [ ] **Step 2: Écrire les requêtes GraphQL**

Remplacer entièrement `src/github/queries.rs` :

```rust
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
```

- [ ] **Step 3: Écrire les tests du client qui échouent**

Remplacer entièrement `src/github/mod.rs` par le module d'en-tête et les tests :

```rust
//! Client GraphQL de GitHub.

pub mod dto;
pub mod queries;

#[cfg(test)]
mod tests {
    use super::*;

    const REPONSE: &str = include_str!("../../tests/fixtures/list.json");

    /// Sert une seule réponse HTTP figée et rend l'adresse à viser.
    ///
    /// Un vrai serveur local plutôt qu'un client simulé : c'est le classement
    /// des réponses — code, en-têtes, corps — qui est testé ici, donc il faut
    /// que `reqwest` fasse réellement le trajet.
    async fn serveur(statut: &str, entetes: &[(&str, &str)], corps: &str) -> String {
        let ecoute = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("un port libre doit être disponible");
        let adresse = ecoute.local_addr().expect("adresse locale");

        let mut reponse = format!(
            "HTTP/1.1 {statut}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            corps.len()
        );
        for (nom, valeur) in entetes {
            reponse.push_str(&format!("{nom}: {valeur}\r\n"));
        }
        reponse.push_str("\r\n");
        reponse.push_str(corps);

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut flux, _) = ecoute.accept().await.expect("connexion acceptée");
            let mut tampon = [0u8; 8192];
            // La requête est lue puis ignorée : seule la réponse compte.
            let _ = flux.read(&mut tampon).await;
            let _ = flux.write_all(reponse.as_bytes()).await;
            let _ = flux.flush().await;
        });

        format!("http://{adresse}/graphql")
    }

    async fn appel(
        statut: &str,
        entetes: &[(&str, &str)],
        corps: &str,
    ) -> Result<crate::model::ListPage, GithubError> {
        let point = serveur(statut, entetes, corps).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        client
            .fetch_pull_requests(&["author:@me".to_string()], 50)
            .await
    }

    #[tokio::test]
    async fn une_reponse_reussie_donne_la_liste_traduite() {
        let page = appel("200 OK", &[], REPONSE).await.expect("succès attendu");
        assert_eq!(page.pull_requests.len(), 5);
        assert_eq!(page.pull_requests[0].key.number, 42);
        assert_eq!(page.rate_limit.expect("solde présent").remaining, 4987);
    }

    #[tokio::test]
    async fn un_tableau_errors_reprend_les_messages_tels_quels() {
        let corps = r#"{"data":null,"errors":[{"message":"Could not resolve to a Repository"},{"message":"Field 'foo' doesn't exist"}]}"#;
        let erreur = appel("200 OK", &[], corps).await.expect_err("erreur attendue");
        assert_eq!(
            erreur.to_string(),
            "Could not resolve to a Repository · Field 'foo' doesn't exist"
        );
    }

    #[tokio::test]
    async fn une_reponse_401_est_un_jeton_refuse() {
        let erreur = appel("401 Unauthorized", &[], r#"{"message":"Bad credentials"}"#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Unauthorized));
        assert_eq!(
            erreur.to_string(),
            "Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler."
        );
    }

    #[tokio::test]
    async fn une_reponse_403_sans_en_tete_est_un_manque_de_droits() {
        let erreur = appel("403 Forbidden", &[], r#"{"message":"Resource not accessible"}"#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Forbidden));
        assert_eq!(
            erreur.to_string(),
            "Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`."
        );
    }

    #[tokio::test]
    async fn une_reponse_403_avec_en_tete_de_reinitialisation_est_une_limite_d_appels() {
        // 1 787 148 720 = 2026-08-30T10:12:00Z
        let erreur = appel(
            "403 Forbidden",
            &[("x-ratelimit-reset", "1787148720")],
            r#"{"message":"API rate limit exceeded"}"#,
        )
        .await
        .expect_err("erreur attendue");
        match erreur {
            GithubError::RateLimited { reset_at } => assert_eq!(
                reset_at.expect("heure de reprise").to_rfc3339(),
                "2026-08-30T10:12:00+00:00"
            ),
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[tokio::test]
    async fn un_corps_tronque_est_une_reponse_illisible() {
        let erreur = appel("200 OK", &[], r#"{"data":{"search":{"nodes":["#)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::Malformed));
    }

    #[tokio::test]
    async fn une_reponse_5xx_rapporte_son_code() {
        let erreur = appel("502 Bad Gateway", &[], "").await.expect_err("erreur attendue");
        assert_eq!(erreur.to_string(), "GitHub a répondu 502.");
    }

    #[test]
    fn la_chaine_de_recherche_joint_les_filtres() {
        assert_eq!(
            search_query(&["author:@me".to_string(), "is:open".to_string()]),
            "author:@me is:open"
        );
    }
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --lib github:: 2>&1 | head -20`
Expected: FAIL à la compilation, `cannot find type GithubError in this scope`.

- [ ] **Step 5: Écrire le client**

Remplacer l'en-tête de `src/github/mod.rs` — commentaire de module et déclarations de modules — par ce qui suit, en laissant le module de tests de l'étape 3 à la fin du fichier :

```rust
//! Client GraphQL de GitHub.
//!
//! Un seul point d'entrée HTTP. Le client classe chaque réponse, et c'est ce
//! classement qui pilote le traitement des erreurs décrit en
//! `docs/specs/05-erreurs-et-tests.md`.

// `fetch_detail` est appelée par la spec 03. Un attribut interne doit précéder
// tout élément du fichier, déclarations de modules comprises.
#![allow(dead_code)]

pub mod dto;
pub mod queries;

use chrono::{DateTime, TimeZone, Utc};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

use crate::model::ListPage;

const ENDPOINT: &str = "https://api.github.com/graphql";

/// En-tête que GitHub renvoie avec un refus pour limite d'appels : c'est lui
/// qui distingue une limite atteinte d'un simple manque de droits.
const RESET_HEADER: &str = "x-ratelimit-reset";

#[derive(Debug, Error)]
pub enum GithubError {
    /// Réponse 200 accompagnée d'un tableau `errors`. Le message de GitHub
    /// est repris tel quel : il dit quoi faire mieux qu'un message maison.
    #[error("{0}")]
    Api(String),
    #[error("Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler.")]
    Unauthorized,
    #[error("Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`.")]
    Forbidden,
    /// L'heure de reprise est portée par la variante et non par le message :
    /// composer « limite d'appels atteinte, reprise à 14 h 32 » est une
    /// décision d'affichage, donc le travail de `app`, à la spec 05.
    #[error("Limite d'appels atteinte.")]
    RateLimited { reset_at: Option<DateTime<Utc>> },
    #[error("GitHub a répondu {0}.")]
    Http(u16),
    #[error("Réponse illisible de GitHub.")]
    Malformed,
    /// Aucun détail de `reqwest` n'est repris : ses messages peuvent citer
    /// l'URL et les en-têtes, où voyage le jeton.
    #[error("Réseau injoignable.")]
    Transport,
    #[error("Pull request introuvable.")]
    NotFound,
}

/// Enveloppe commune à toute réponse GraphQL.
#[derive(Deserialize)]
struct Envelope<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

/// Client HTTP prêt à l'emploi. Il porte le jeton dans ses en-têtes par
/// défaut : c'est le seul endroit du programme où le jeton est conservé, et il
/// n'en ressort jamais.
pub struct Client {
    http: reqwest::Client,
    endpoint: String,
}

impl Client {
    pub fn new(token: &str) -> Result<Self, GithubError> {
        Self::with_endpoint(token, ENDPOINT)
    }

    /// Point d'entrée réglable : les tests visent un serveur local.
    pub fn with_endpoint(token: &str, endpoint: &str) -> Result<Self, GithubError> {
        let mut entetes = HeaderMap::new();

        let mut autorisation =
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| GithubError::Unauthorized)?;
        // Marqué sensible : `reqwest` ne l'écrit pas dans ses traces.
        autorisation.set_sensitive(true);
        entetes.insert(AUTHORIZATION, autorisation);
        entetes.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("owl/", env!("CARGO_PKG_VERSION"))),
        );

        let http = reqwest::Client::builder()
            .default_headers(entetes)
            .build()
            .map_err(|_| GithubError::Transport)?;

        Ok(Self {
            http,
            endpoint: endpoint.to_string(),
        })
    }

    /// Envoie un document et classe la réponse. Les quatre issues de la spec
    /// sont décidées ici, et nulle part ailleurs.
    async fn execute<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T, GithubError> {
        let reponse = self
            .http
            .post(&self.endpoint)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|_| GithubError::Transport)?;

        let statut = reponse.status();
        let reprise = reset_at(reponse.headers());
        let corps = reponse.text().await.map_err(|_| GithubError::Transport)?;

        if statut == StatusCode::UNAUTHORIZED {
            return Err(GithubError::Unauthorized);
        }
        if statut == StatusCode::FORBIDDEN || statut == StatusCode::TOO_MANY_REQUESTS {
            // C'est l'en-tête de réinitialisation qui tranche : sans lui, le
            // refus porte sur les droits, pas sur le nombre d'appels.
            return Err(match reprise {
                Some(_) => GithubError::RateLimited { reset_at: reprise },
                None => GithubError::Forbidden,
            });
        }
        if !statut.is_success() {
            return Err(GithubError::Http(statut.as_u16()));
        }

        let enveloppe: Envelope<T> =
            serde_json::from_str(&corps).map_err(|_| GithubError::Malformed)?;

        if let Some(erreurs) = enveloppe.errors.filter(|liste| !liste.is_empty()) {
            let message = erreurs
                .into_iter()
                .map(|erreur| erreur.message)
                .collect::<Vec<_>>()
                .join(" · ");
            return Err(GithubError::Api(message));
        }

        enveloppe.data.ok_or(GithubError::Malformed)
    }

    /// Ramène les pull requests correspondant aux filtres, avec le solde
    /// d'appels lu au passage.
    ///
    /// Un solde à zéro n'est pas une erreur : les données de cette réponse
    /// sont bonnes. La suspension du rafraîchissement qu'il déclenche
    /// appartient à `05-erreurs-et-tests.md`.
    pub async fn fetch_pull_requests(
        &self,
        filters: &[String],
        page_size: u16,
    ) -> Result<ListPage, GithubError> {
        let variables = json!({ "q": search_query(filters), "n": page_size });
        let donnees: dto::ListData = self.execute(queries::LIST, variables).await?;
        Ok(donnees.to_list_page())
    }
}

/// Heure de réinitialisation portée par l'en-tête de limite d'appels, en
/// secondes depuis l'époque.
fn reset_at(entetes: &HeaderMap) -> Option<DateTime<Utc>> {
    let brut = entetes.get(RESET_HEADER)?.to_str().ok()?;
    let secondes: i64 = brut.trim().parse().ok()?;
    Utc.timestamp_opt(secondes, 0).single()
}

/// Assemble la chaîne de recherche.
///
/// Bouchon jusqu'à `docs/specs/02-filtres.md` : les filtres sont déjà écrits
/// dans la syntaxe de GitHub, il suffit de les joindre. `filter::build_query`
/// prendra sa place et ajoutera `is:pr` et `sort:updated-desc`. Rien n'est
/// ajouté ici, pour ne pas dupliquer une règle qui appartient à la spec 02.
fn search_query(filters: &[String]) -> String {
    filters.join(" ")
}
```

Attention à l'ordre : `#![allow(dead_code)]` est un attribut interne, il doit précéder tout élément du fichier — `pub mod dto;` compris. Et `pub mod dto;` / `pub mod queries;` ne doivent apparaître qu'une fois : cet en-tête remplace celui de l'étape 3, il ne s'y ajoute pas.

- [ ] **Step 6: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test --lib github:: 2>&1 | tail -25`
Expected: les huit tests du client passent, ainsi que ceux de `dto`. Le reste du projet ne compile plus : `main` appelle encore la fonction libre disparue.

- [ ] **Step 7: Faire porter l'erreur typée à `app`**

Dans `src/app.rs`, ajouter l'import et changer le type du résultat :

```rust
use crate::github::GithubError;
```

```rust
    /// Résultat d'une demande réseau.
    Data {
        generation: Generation,
        result: Result<ListPage, GithubError>,
    },
```

Le bras `Err` est inchangé dans son intention — le message de GitHub reste repris tel quel — mais passe par l'affichage du type d'erreur :

```rust
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(erreur) => self.status = erreur.to_string(),
```

- [ ] **Step 8: Adapter les deux tests d'erreur de `app`**

Dans le module de tests de `src/app.rs`, les deux tests qui simulaient une panne réseau passent désormais la variante correspondante. Remplacer `result: Err("Réseau injoignable".to_string())` par `result: Err(GithubError::Transport)` dans `une_erreur_laisse_la_liste_affichee` et `un_succes_efface_l_erreur_en_cours`, ajouter `use crate::github::GithubError;` au module de tests, et ajuster les deux textes attendus, qui gagnent le point final du message :

```rust
        assert_eq!(app.status, "Réseau injoignable.", "message repris tel quel");
```

```rust
        assert_eq!(
            app.status_line(),
            "Réseau injoignable. · q quitter · r rafraîchir",
            "aucune heure : aucun rafraîchissement n'a encore réussi"
        );
```

Dans `un_succes_efface_l_erreur_en_cours`, le `contains` porte sur `"Réseau injoignable"` : il reste valable tel quel.

- [ ] **Step 9: Construire un client unique dans `main`**

Dans `src/main.rs`, le jeton ne sert plus qu'à construire le client, une seule fois, avant la boucle. Dans `run`, remplacer la ligne `let jeton = Arc::new(jeton);` par :

```rust
    // Le client est construit une fois pour toutes : il porte le jeton dans
    // ses en-têtes, et c'est le seul endroit du programme où le jeton reste.
    let client = Arc::new(github::Client::new(jeton.expose())?);
```

Puis remplacer les deux appels `execute_command(commande, &envoi, &jeton)` par `execute_command(commande, &envoi, &client)`, et la fonction elle-même :

```rust
fn execute_command(
    commande: Command,
    envoi: &UnboundedSender<Event>,
    client: &Arc<github::Client>,
) -> bool {
    match commande {
        Command::Quit => return true,
        Command::Fetch {
            generation,
            filters,
            page_size,
        } => {
            let envoi = envoi.clone();
            let client = client.clone();
            tokio::spawn(async move {
                let resultat = client.fetch_pull_requests(&filters, page_size).await;
                let _ = envoi.send(Event::Data {
                    generation,
                    result: resultat,
                });
            });
        }
    }
    false
}
```

Le commentaire de la fonction reste vrai : c'est toujours le seul endroit où passe l'accès à GitHub, et ni `app` ni `ui` ne le voient.

- [ ] **Step 10: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe. Si `clippy` signale un `use` devenu inutile dans `main.rs` — `token::Token` n'est plus enveloppé dans un `Arc` de la boucle — le retirer.

- [ ] **Step 11: Reporter les décisions dans la spec**

Dans `docs/specs/01-modele-et-donnees.md`, remplacer la section « Note d'implémentation » par :

```markdown
## Note d'implémentation

Les fondations laissaient un bouchon : `github::fetch_pull_requests` renvoyait une
liste vide sans toucher au réseau, et son erreur était un simple `String`. Cette
spec le remplace par `github::Client`, dont l'erreur est le type `thiserror`
`GithubError`. Trois conséquences :

- `app::Event::Data` porte `Result<ListPage, GithubError>`. `app` connaît donc le
  type d'erreur de `github`, ce que les règles de dépendance autorisent : elles
  interdisent à `github` de connaître `app`, pas l'inverse, et `app` ne gagne
  aucun appel réseau au passage.
- Le solde d'appels voyage avec la liste, dans `ListPage { pull_requests,
  rate_limit }`. Un `rateLimit.remaining` nul dans une réponse réussie n'est pas
  une erreur : les données sont rendues, le solde est transmis, et la suspension
  du rafraîchissement reste le sujet de `05-erreurs-et-tests.md`.
  `GithubError::RateLimited` est réservé au refus de GitHub, reconnu à la réponse
  403 accompagnée d'un en-tête de réinitialisation.
- La chaîne de recherche est, jusqu'à `02-filtres.md`, la simple jointure des
  filtres des réglages. `is:pr` n'est pas ajouté ici : c'est une règle de
  `build_query`, et la dupliquer serait la faire vivre à deux endroits.

La mutation de fusion est posée en constante dans `github::queries`, et n'est
appelée par aucune fonction à ce stade : son déclenchement, comme le choix de la
méthode, appartient à `04-fusion.md`.
```

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml Cargo.lock src/github/queries.rs src/github/mod.rs src/app.rs src/main.rs docs/specs/01-modele-et-donnees.md
git commit -m "Branche le vrai client GraphQL sur la liste"
```

---

### Task 4: Lecture et traduction de la réponse de détail

**Files:**
- Create: `tests/fixtures/detail.json`
- Modify: `src/github/dto.rs`
- Modify: `src/github/mod.rs`
- Modify: `docs/specs/01-modele-et-donnees.md`
- Modify: `docs/suivi/DETTE.md`

**Interfaces:**
- Consumes: `dto::Actor`, `dto::CommitConnection`, `dto::checks_from_rollup` de la tâche 2 ; `Client::execute` et `queries::DETAIL` de la tâche 3 ; `model::PrKey::owner` et `model::PrKey::name` de la tâche 1.
- Produces :
  - `dto::DetailData { repository: Option<RepositoryDetail> }` et sa descendance.
  - `dto::PullRequestDetail::to_detail(&self, summary: model::PrSummary) -> model::PrDetail`.
  - `dto::ContextNode::to_check_run(&self) -> Option<model::CheckRun>` — traduit les deux formes de vérification.
  - `Client::fetch_detail(&self, summary: &model::PrSummary) -> Result<model::PrDetail, GithubError>`.
- Le `ContextNode` vide de la tâche 2 est remplacé par sa version complète.

- [ ] **Step 1: Enregistrer la réponse de détail**

Créer `tests/fixtures/detail.json`. Les contextes couvrent les cinq formes qui arrivent vraiment : un `CheckRun` terminé et réussi, un `CheckRun` en cours donc sans conclusion, un `CheckRun` terminé et neutre, un `StatusContext` en échec, un `StatusContext` attendu, et un nœud d'aucune des deux formes. Une relecture sans `submittedAt` est également présente : c'est une relecture en attente.

```json
{
  "data": {
    "repository": {
      "pullRequest": {
        "id": "PR_kwDOABCD12345",
        "body": "Ajoute la fenêtre de fusion et ses raccourcis.",
        "headRefName": "feat/fusion",
        "baseRefName": "develop",
        "additions": 214,
        "deletions": 37,
        "commits": {
          "nodes": [
            {
              "commit": {
                "statusCheckRollup": {
                  "contexts": {
                    "nodes": [
                      {
                        "name": "build",
                        "conclusion": "SUCCESS",
                        "status": "COMPLETED",
                        "detailsUrl": "https://github.com/moi/owl/actions/runs/1"
                      },
                      {
                        "name": "clippy",
                        "conclusion": null,
                        "status": "IN_PROGRESS",
                        "detailsUrl": "https://github.com/moi/owl/actions/runs/2"
                      },
                      {
                        "name": "documentation",
                        "conclusion": "SKIPPED",
                        "status": "COMPLETED",
                        "detailsUrl": null
                      },
                      {
                        "context": "ci/ancien-service",
                        "state": "FAILURE",
                        "targetUrl": "https://ancien.example/build/9"
                      },
                      {
                        "context": "ci/attente",
                        "state": "EXPECTED",
                        "targetUrl": null
                      },
                      {}
                    ]
                  }
                }
              }
            }
          ]
        },
        "reviews": {
          "nodes": [
            {
              "author": { "login": "camille" },
              "state": "APPROVED",
              "body": "Bon pour moi.",
              "submittedAt": "2026-08-30T08:00:00Z"
            },
            {
              "author": null,
              "state": "CHANGES_REQUESTED",
              "body": "Il manque un test.",
              "submittedAt": "2026-08-29T15:30:00Z"
            },
            {
              "author": { "login": "moi" },
              "state": "PENDING",
              "body": "",
              "submittedAt": null
            }
          ]
        },
        "comments": {
          "nodes": [
            {
              "author": { "login": "moi" },
              "body": "Je rebase et je fusionne.",
              "createdAt": "2026-08-30T08:15:00Z"
            }
          ]
        },
        "files": {
          "nodes": [
            { "path": "src/ui/merge.rs", "additions": 180, "deletions": 2 },
            { "path": "src/app.rs", "additions": 34, "deletions": 35 }
          ]
        }
      }
    }
  }
}
```

- [ ] **Step 2: Écrire les tests qui échouent**

Ajouter dans le module de tests de `src/github/dto.rs` :

```rust
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
        assert_eq!(detail.body, "Ajoute la fenêtre de fusion et ses raccourcis.");
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
        assert_eq!(detail.reviews.len(), 2, "la relecture en attente est ignorée");
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
```

- [ ] **Step 3: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test --lib github::dto 2>&1 | head -20`
Expected: FAIL à la compilation, `cannot find type DetailData in this scope`.

- [ ] **Step 4: Écrire les types de détail et leur traduction**

Dans `src/github/dto.rs`, remplacer d'abord le `ContextNode` vide de la tâche 2 par sa version complète, puis ajouter le reste après les types de liste. Compléter l'import de `model` avec `ChangedFile, CheckRun, Comment, PrDetail, Review`.

```rust
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
```

Compléter aussi l'import de `PrSummary` déjà présent : la liste devient

```rust
use crate::model::{
    ChangedFile, CheckRun, ChecksState, Comment, ListPage, MergeableState, PrDetail, PrKey,
    PrSummary, RateLimit, RepoMergeRules, Review, ReviewState, AUTEUR_INCONNU,
};
```

- [ ] **Step 5: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test --lib github::dto 2>&1 | tail -25`
Expected: les onze tests de `dto` passent.

- [ ] **Step 6: Ajouter la requête de détail au client**

Dans `src/github/mod.rs`, compléter l'import — `use crate::model::{ListPage, PrDetail, PrSummary};` — et ajouter la méthode après `fetch_pull_requests` :

```rust
    /// Détail d'une seule pull request, lancé à l'ouverture de la vue détail.
    ///
    /// Le résumé déjà affiché est repris tel quel : la requête de détail ne
    /// renvoie aucun de ses champs. Elle apporte en revanche l'identifiant
    /// GraphQL, nécessaire à la fusion.
    pub async fn fetch_detail(&self, summary: &PrSummary) -> Result<PrDetail, GithubError> {
        let variables = json!({
            "owner": summary.key.owner(),
            "name": summary.key.name(),
            "number": summary.key.number,
        });
        let donnees: dto::DetailData = self.execute(queries::DETAIL, variables).await?;
        donnees
            .repository
            .and_then(|depot| depot.pull_request)
            .map(|pr| pr.to_detail(summary.clone()))
            .ok_or(GithubError::NotFound)
    }
```

- [ ] **Step 7: Ajouter le test du client sur le détail**

Dans le module de tests de `src/github/mod.rs` :

```rust
    #[tokio::test]
    async fn le_client_ramene_le_detail_d_une_pull_request() {
        const DETAIL: &str = include_str!("../../tests/fixtures/detail.json");
        let point = serveur("200 OK", &[], DETAIL).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        let resume = crate::model::PrSummary {
            key: crate::model::PrKey {
                repo: "moi/owl".to_string(),
                number: 42,
            },
            title: "Ajoute la fenêtre de fusion".to_string(),
            author: "moi".to_string(),
            url: "https://github.com/moi/owl/pull/42".to_string(),
            is_draft: false,
            checks: crate::model::ChecksState::Success,
            review: crate::model::ReviewState::Approved,
            mergeable: crate::model::MergeableState::Mergeable,
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: crate::model::RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            },
        };

        let detail = client.fetch_detail(&resume).await.expect("succès attendu");
        assert_eq!(detail.node_id, "PR_kwDOABCD12345");
        assert_eq!(detail.checks.len(), 5);
        assert_eq!(detail.summary, resume);
    }

    #[tokio::test]
    async fn une_pull_request_absente_de_la_reponse_est_introuvable() {
        let point = serveur("200 OK", &[], r#"{"data":{"repository":null}}"#).await;
        let client = Client::with_endpoint("jeton-de-test", &point).expect("client construit");
        let resume = crate::model::PrSummary {
            key: crate::model::PrKey {
                repo: "moi/owl".to_string(),
                number: 1,
            },
            title: String::new(),
            author: "moi".to_string(),
            url: String::new(),
            is_draft: false,
            checks: crate::model::ChecksState::None,
            review: crate::model::ReviewState::None,
            mergeable: crate::model::MergeableState::Unknown,
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: crate::model::RepoMergeRules {
                squash: true,
                merge: true,
                rebase: true,
                delete_branch_on_merge: false,
            },
        };
        let erreur = client
            .fetch_detail(&resume)
            .await
            .expect_err("erreur attendue");
        assert!(matches!(erreur, GithubError::NotFound));
    }
```

- [ ] **Step 8: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test --lib github 2>&1 | tail -25`
Expected: tous les tests de `github` passent, treize en tout.

- [ ] **Step 9: Reporter les décisions dans la spec**

Dans `docs/specs/01-modele-et-donnees.md`, ajouter à la fin de la section « Requête de détail » :

```markdown
Les états qui n'existent que dans la vue détail sont traduits ainsi :

| Situation | Traduction |
|---|---|
| `CheckRun` dont `status` n'est pas `COMPLETED` | `Pending` — la conclusion n'existe pas encore |
| `CheckRun` de conclusion `SUCCESS` | `Success` |
| `CheckRun` de conclusion `NEUTRAL` ou `SKIPPED` | `None` — aucun verdict sur le code |
| `CheckRun` de toute autre conclusion | `Failure` |
| `StatusContext` | table du `statusCheckRollup`, sur son champ `state` |
| Nœud d'aucune des deux formes | ignoré |
| Relecture d'état `APPROVED` / `CHANGES_REQUESTED` | `Approved` / `ChangesRequested` |
| Relecture d'un autre état (`COMMENTED`, `DISMISSED`, `PENDING`) | `None` |
| Relecture sans `submittedAt` (en attente, jamais soumise) | ignorée |

La ligne « … et N de plus » n'est pas réalisable avec cette requête : elle ne
demande aucun `totalCount`, et une liste bornée à vingt éléments qui en renvoie
vingt est indiscernable d'une liste complète de vingt. La spec 03, qui possède cet
affichage, tranchera entre ajouter les `totalCount` à la requête et abandonner la
ligne.
```

- [ ] **Step 10: Consigner les points différés**

Dans `docs/suivi/DETTE.md`, ajouter deux entrées en haut, au-dessus de l'entrée existante et sous le séparateur `---`, au format du fichier :

```markdown
### La troncature des listes de la vue détail n'est pas mesurable

- **Origine** : plan `2026-09-02-modele-et-donnees`, tâche 4.
- **Ce qui est différé** : la requête de détail borne les listes — vingt relectures, vingt commentaires, cent fichiers — mais ne demande aucun `totalCount`. Impossible, donc, de savoir si une liste est tronquée, alors que `01-modele-et-donnees.md` prévoit une ligne « … et N de plus ».
- **Pourquoi** : la ligne est un élément d'affichage, et l'affichage de la vue détail appartient à `03-affichage-et-navigation.md`. Ajouter des `totalCount` maintenant serait modifier la requête de la spec 01 pour un besoin que personne ne consomme encore.
- **Ce qu'il faudrait faire** : à la spec 03, ajouter `totalCount` aux trois connexions de la requête de détail, le porter dans `PrDetail` — trois champs, ou un compte par liste — et composer la ligne dans `app`. Ou décider que la ligne disparaît, et retirer la phrase de la spec 01.

### La chaîne de recherche est une jointure de filtres

- **Origine** : plan `2026-09-02-modele-et-donnees`, tâche 3.
- **Ce qui est différé** : `github::search_query` joint les filtres des réglages avec une espace et n'ajoute rien. Ni `is:pr`, ni `sort:updated-desc`. La requête ramène donc aussi des issues, écartées à la traduction, et l'ordre des résultats est celui de GitHub par défaut.
- **Pourquoi** : `is:pr` et `sort:updated-desc` sont des règles de `filter::build_query`, définies par `02-filtres.md`. Les écrire dans `github` les ferait vivre à deux endroits, et la spec 02 les déplacerait aussitôt.
- **Ce qu'il faudrait faire** : à la spec 02, écrire `filter::Filter` et `filter::build_query`, faire porter à `Config::filters` des `Filter` plutôt que des chaînes, et remplacer l'appel à `search_query` par `filter::build_query`. `search_query` disparaît alors.
```

- [ ] **Step 11: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe.

- [ ] **Step 12: Commit**

```bash
git add tests/fixtures/detail.json src/github/dto.rs src/github/mod.rs docs/specs/01-modele-et-donnees.md docs/suivi/DETTE.md
git commit -m "Traduit la réponse de détail et ses deux formes de vérification"
```

---

### Task 5: Vérification sur le vrai GitHub

**Files:** aucun changement de code attendu. Si la vérification révèle un écart, il est corrigé dans cette tâche, test d'abord, avec la réponse réelle ajoutée à la fixture concernée.

**Interfaces:**
- Consumes: tout ce que les tâches 1 à 4 produisent.
- Produces: rien. Cette tâche est une porte, pas une livraison.

**Note sur la vérification visuelle :** `owl` est un outil de terminal, sans interface web. `claude-in-chrome` ne s'applique donc pas, et aucun compte de test navigateur n'est nécessaire. La spec 01 n'ajoute par ailleurs aucun élément d'interface : elle remplit la liste déjà dessinée aux fondations. Ce qui peut être vérifié sans terminal interactif l'est ici, à l'étape 1 : que GitHub accepte bien le document GraphQL. Le coup d'œil à l'écran, lui, demande un terminal interactif qu'une session automatisée ne peut pas prendre — il n'est ni demandé à l'humain ni mentionné dans le rapport final.

- [ ] **Step 1: Vérifier que GitHub accepte la requête de liste**

Non interactif, donc exécutable directement. Extraire la constante et l'envoyer à l'API réelle :

```bash
python3 -c "
import re
s = open('src/github/queries.rs').read()
print(re.search(r'pub const LIST: &str = r#\"(.*?)\"#;', s, re.S).group(1))
" > /tmp/owl-list.graphql
gh api graphql -F q='author:@me is:open' -F n=5 -F query=@/tmp/owl-list.graphql | head -40
```

Expected: un objet JSON avec `data.search.nodes` et `data.rateLimit`. Si GitHub renvoie un tableau `errors`, le document de `queries.rs` s'écarte de ce que l'API accepte : c'est un défaut à corriger, spec 01 mise à jour dans le même commit.

Si `gh` est absent ou non connecté, cette étape n'est pas réalisable : passer à la suivante sans poser de question et sans en faire mention dans le rapport final. Les tests contre le serveur local couvrent déjà le classement des réponses ; seule la validation du document par GitHub manque alors.

- [ ] **Step 2: Traiter un écart révélé par l'étape 1**

Reproduire d'abord en test. Une réponse réelle qui surprend s'ajoute à `tests/fixtures/list.json` ou `tests/fixtures/detail.json` — ces fichiers se complètent, on ne contourne pas un cas gênant — puis le test échoue, puis la traduction est corrigée. Si l'écart porte sur une décision d'affichage, il appartient à `03-affichage-et-navigation.md` : le consigner dans `docs/suivi/DETTE.md` plutôt que de le régler ici.

- [ ] **Step 3: Vérifier les critères de réussite de la spec, un par un**

```bash
cargo test 2>&1 | tail -30
```

Chaque critère de `01-modele-et-donnees.md` doit pointer sur un test nommé :

| Critère de la spec | Test |
|---|---|
| Une réponse enregistrée se traduit en `Vec<PrSummary>` exact, états compris | `la_reponse_enregistree_donne_la_premiere_pull_request_exacte`, `les_etats_sont_traduits_un_a_un` |
| Un nœud de type issue mélangé est ignoré sans faire échouer la traduction | `un_noeud_d_issue_est_ignore_sans_faire_echouer_la_traduction` |
| Une PR sans aucune CI donne `ChecksState::None`, distinct de `Pending` | `une_pr_sans_aucune_ci_donne_none_et_non_pending` |
| Les deux formes de vérification produisent des entrées équivalentes | `les_deux_formes_de_verification_donnent_des_entrees_equivalentes` |

- [ ] **Step 4: Lancer les quatre commandes une dernière fois**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe.

- [ ] **Step 5: Commit s'il y a eu des corrections**

```bash
git add -A
git commit -m "Corrige les écarts relevés à la vérification sur le vrai GitHub"
```

S'il n'y a rien à corriger, aucun commit : la tâche est une porte franchie, pas un changement.

- [ ] **Step 6: Ouvrir la pull request**

Pousser la branche et ouvrir une pull request **vers `develop`**, jamais vers `main`. Titre et corps en français ; le corps renvoie à `docs/specs/01-modele-et-donnees.md` et liste les critères de réussite couverts.

```bash
git push -u origin feat/modele-et-donnees
gh pr create --base develop --title "Modèle métier et accès aux données" --body "<corps en français>"
```

- [ ] **Step 7: Rapporter**

Le rapport final ne raconte pas le travail fait. Il donne uniquement, s'il y en a, ce que l'humain doit faire lui-même — une variable d'environnement à poser, un outil à installer — et le lien de la pull request. Rien sur `docs/suivi/DETTE.md`, rien sur les vérifications à l'œil, rien sur les manipulations git. S'il n'y a aucune action humaine à mener, le dire en une phrase avec le lien de la pull request.
