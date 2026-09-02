//! Types métier. Ne dépend ni du réseau ni du terminal.
//!
//! Ces types sont ceux de `docs/specs/01-modele-et-donnees.md`. Ils ne
//! connaissent pas le vocabulaire de GitHub : la traduction est faite par
//! `github::dto`, seul endroit qui voit passer un `SUCCESS` ou un
//! `nameWithOwner`.

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

/// Méthode de fusion. Le vocabulaire de GitHub — `SQUASH`, `REBASE`, `MERGE` —
/// reste chez `github` ; ici, seule la notion métier.
///
/// Ce type vit dans `model` et non dans `config` parce que `github` en a
/// besoin pour la mutation : le sens des dépendances interdit à `github` de
/// connaître les réglages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Squash,
    Rebase,
    Merge,
}

impl RepoMergeRules {
    /// Méthodes autorisées par le dépôt, dans l'ordre imposé par la spec :
    /// écrasement, rebasage, commit de fusion. C'est aussi l'ordre de la
    /// fenêtre de confirmation, et celui du repli quand la méthode préférée
    /// n'est pas autorisée.
    pub fn allowed(&self) -> Vec<MergeMethod> {
        let mut methodes = Vec::new();
        if self.squash {
            methodes.push(MergeMethod::Squash);
        }
        if self.rebase {
            methodes.push(MergeMethod::Rebase);
        }
        if self.merge {
            methodes.push(MergeMethod::Merge);
        }
        methodes
    }
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

    /// Règles où tout est refusé, base des cas ci-dessous.
    fn rien_autorise() -> RepoMergeRules {
        RepoMergeRules {
            squash: false,
            merge: false,
            rebase: false,
            delete_branch_on_merge: false,
        }
    }

    #[test]
    fn un_depot_sans_methode_n_en_autorise_aucune() {
        assert!(rien_autorise().allowed().is_empty());
    }

    #[test]
    fn un_depot_qui_n_autorise_que_l_ecrasement_ne_rend_que_l_ecrasement() {
        let regles = RepoMergeRules {
            squash: true,
            ..rien_autorise()
        };
        assert_eq!(regles.allowed(), vec![MergeMethod::Squash]);
    }

    #[test]
    fn les_methodes_sont_rendues_dans_l_ordre_ecrasement_rebasage_fusion() {
        let regles = RepoMergeRules {
            squash: true,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        };
        assert_eq!(
            regles.allowed(),
            vec![MergeMethod::Squash, MergeMethod::Rebase, MergeMethod::Merge]
        );
    }
}
