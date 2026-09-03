//! Dessin de la fenêtre de confirmation de fusion.
//!
//! Aucune décision : les lignes arrivent déjà repliées et teintées par `app`,
//! à la largeur qu'elle a reçue. Ici, seulement la mesure, le centrage,
//! l'effacement du fond et le cadre.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{MergeLine, MergeRender};
use crate::ui::color;

/// Marge intérieure de chaque côté, en plus des deux colonnes de bordure.
const MARGIN: u16 = 2;

pub fn draw(frame: &mut Frame, area: Rect, render: &MergeRender) {
    let content = render
        .lines
        .iter()
        .map(|line| line.text().chars().count())
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
        width,
        height,
    };

    // `Clear` d'abord : sans lui, la liste resterait visible sous la fenêtre.
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(render.title.clone());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text: Vec<Line> = render.lines.iter().map(line).collect();
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

/// Une ligne : les morceaux composés par `app`, chacun avec son ton.
fn line(source: &MergeLine) -> Line<'static> {
    Line::from(
        source
            .cells
            .iter()
            .map(|cell| match cell.tone {
                Some(tone) => Span::styled(cell.text.clone(), Style::default().fg(color(tone))),
                None => Span::raw(cell.text.clone()),
            })
            .collect::<Vec<_>>(),
    )
}
