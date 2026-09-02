//! Aiguillage de dessin. Lit `app` en lecture seule et ne décide de rien.

pub mod detail;
pub mod list;
pub mod merge;

use ratatui::style::Color;
use ratatui::Frame;

use crate::app::{App, Tone, View};

pub fn draw(frame: &mut Frame, app: &App) {
    let zone = frame.area();
    match app.view {
        View::List => list::draw(frame, zone, app),
        View::Detail { .. } => detail::draw(frame, zone, app),
    }
}

pub(crate) fn couleur(ton: Tone) -> Color {
    match ton {
        Tone::Vert => Color::Green,
        Tone::Rouge => Color::Red,
        Tone::Jaune => Color::Yellow,
        Tone::Gris => Color::DarkGray,
    }
}
