//! Client GraphQL de GitHub.

pub mod dto;
pub mod queries;

use crate::model::PullRequest;

/// Ramène les pull requests correspondant aux filtres.
///
/// Bouchon au stade des fondations : renvoie une liste vide sans toucher au
/// réseau, ce qui suffit à faire tourner la boucle d'événements et le
/// mécanisme de générations. Le vrai client est défini par
/// `docs/specs/01-modele-et-donnees.md`, qui remplacera aussi le `String`
/// d'erreur par un type `thiserror`.
pub async fn fetch_pull_requests(
    _token: &str,
    _filters: &[String],
    _page_size: u16,
) -> Result<Vec<PullRequest>, String> {
    Ok(Vec::new())
}
