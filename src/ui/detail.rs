//! Dessin de la vue détail d'une pull request.
//!
//! Une seule zone qui défile. Aucune décision : les lignes et leurs tons sont
//! composés par `app`.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::ui::couleur;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let cadre = Block::default()
        .borders(Borders::ALL)
        .title(" owl — détail ");
    let interieur = cadre.inner(zones[0]);
    frame.render_widget(cadre, zones[0]);

    let lignes: Vec<Line> = app
        .detail_lines(interieur.width)
        .into_iter()
        .map(|ligne| {
            let style = match ligne.tone {
                Some(ton) => Style::default().fg(couleur(ton)),
                None => Style::default(),
            };
            Line::from(Span::styled(ligne.text, style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lignes).scroll((app.detail_scroll(), 0)),
        interieur,
    );

    frame.render_widget(Paragraph::new(app.status_line(zones[1].width)), zones[1]);
}
