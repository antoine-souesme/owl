//! Dessin de la fenêtre de confirmation de fusion.
//!
//! Aucune décision : les lignes arrivent déjà repliées par `app`, à la
//! largeur qu'elle a reçue. Ici, seulement la mesure, le centrage,
//! l'effacement du fond et le cadre.

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::MergeRender;

/// Marge intérieure de chaque côté, en plus des deux colonnes de bordure.
const MARGIN: u16 = 2;

pub fn draw(frame: &mut Frame, area: Rect, render: &MergeRender) {
    let content = render
        .lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let content = content.max(render.title.chars().count());

    // Deux colonnes de bordure, deux marges de chaque côté. `app` a déjà
    // replié les lignes contre la largeur disponible : ce `.min` est un
    // filet de sécurité, pas un repli.
    let width = u16::try_from(content + 2 + 2 * MARGIN as usize)
        .unwrap_or(u16::MAX)
        .min(area.width);
    let height = u16::try_from(render.lines.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height);

    let area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width: width,
        height: height,
    };

    // `Clear` d'abord : sans lui, la liste resterait visible sous la fenêtre.
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(render.title.clone());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = render.lines.join("\n");
    frame.render_widget(
        Paragraph::new(text),
        Rect {
            x: inner.x + MARGIN.min(inner.width),
            y: inner.y,
            width: inner.width.saturating_sub(2 * MARGIN),
            height: inner.height,
        },
    );
}
