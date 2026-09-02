//! Types métier. Ne dépend ni du réseau ni du terminal.

/// Une pull request telle qu'affichée dans la liste.
/// La spec `01-modele-et-donnees.md` étend ce type.
#[derive(Debug, Clone, PartialEq)]
pub struct PullRequest {
    /// Dépôt au format `proprietaire/nom`.
    pub repository: String,
    pub number: u32,
    pub title: String,
}
