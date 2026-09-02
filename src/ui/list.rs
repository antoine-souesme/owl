//! Dessin de la liste des pull requests et de la barre d'état.
//!
//! Aucune décision : les messages et les résumés sont préparés par `app`.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let lignes: Vec<ListItem> = app
        .items
        .iter()
        .map(|pr| ListItem::new(format!("{}#{}  {}", pr.key.repo, pr.key.number, pr.title)))
        .collect();

    let liste = List::new(lignes).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" owl — pull requests "),
    );
    frame.render_widget(liste, zones[0]);

    let barre = Paragraph::new(app.status_line());
    frame.render_widget(barre, zones[1]);
}
