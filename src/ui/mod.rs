//! Aiguillage de dessin. Lit `app` en lecture seule et ne décide de rien.

pub mod detail;
pub mod list;
pub mod merge;

use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    // Une seule vue au stade des fondations. L'aiguillage selon la vue
    // courante est défini par `docs/specs/03-affichage-et-navigation.md`.
    list::draw(frame, frame.area(), app);
}
