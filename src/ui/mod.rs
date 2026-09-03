//! Aiguillage de dessin. Lit `app` en lecture seule et ne décide de rien.

pub mod detail;
pub mod list;
pub mod merge;

use ratatui::style::Color;
use ratatui::Frame;

use crate::app::{App, Tone, View};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match app.view {
        View::List => list::draw(frame, area, app),
        View::Detail { .. } => detail::draw(frame, area, app),
    }
    // Par-dessus la vue courante, et après elle : la fenêtre est modale.
    if let Some(render) = app.merge_render(area.width) {
        merge::draw(frame, area, &render);
    }
}

pub(crate) fn color(tone: Tone) -> Color {
    match tone {
        Tone::Green => Color::Green,
        Tone::Red => Color::Red,
        Tone::Yellow => Color::Yellow,
        Tone::Gray => Color::DarkGray,
    }
}
