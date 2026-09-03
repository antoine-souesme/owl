//! Dessin de la vue détail d'une pull request.
//!
//! Une seule zone qui défile. Aucune décision : les lignes et leurs tons sont
//! composés par `app`.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, DETAIL_TITLE};
use crate::ui::color;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let block = Block::default().borders(Borders::ALL).title(DETAIL_TITLE);
    let inner = block.inner(areas[0]);
    frame.render_widget(block, areas[0]);

    let lines: Vec<Line> = app
        .detail_lines(inner.width)
        .into_iter()
        .map(|line| {
            let style = match line.tone {
                Some(tone) => Style::default().fg(color(tone)),
                None => Style::default(),
            };
            Line::from(Span::styled(line.text, style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).scroll((app.detail_scroll(), 0)),
        inner,
    );

    frame.render_widget(Paragraph::new(app.status_line(areas[1].width)), areas[1]);
}
