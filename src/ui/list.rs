//! Dessin de la liste des pull requests et de la barre d'état.
//!
//! Aucune décision : les pictogrammes, les colonnes, la troncature et les
//! messages sont composés par `app`. Ici, seulement la mise en page, la
//! couleur, et le curseur de sélection.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, ListRender, ListRow};
use crate::ui::color;

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" owl — pull requests ");
    // La largeur utile est celle de l'intérieur du cadre : c'est elle que
    // `app` doit connaître pour décider de la troncature.
    let inner = block.inner(areas[0]);
    frame.render_widget(block, areas[0]);

    match app.list_render(inner.width) {
        ListRender::Rows(lines) => {
            let items: Vec<ListItem> = lines.into_iter().map(item).collect();
            // L'état de sélection est reconstruit à chaque dessin depuis
            // `app.selected` : c'est lui qui fait défiler la liste quand elle
            // dépasse la hauteur, et `ui` ne retient rien entre deux dessins.
            let mut state = ListState::default().with_selected(Some(app.selected));
            let list =
                List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(list, inner, &mut state);
        }
        ListRender::Empty(lines) => {
            let text: Vec<Line> = lines.into_iter().map(Line::from).collect();
            frame.render_widget(Paragraph::new(text), inner);
        }
        ListRender::TooNarrow(message) => {
            frame.render_widget(Paragraph::new(message), inner);
        }
    }

    // La largeur de la barre lui est donnée : c'est `app` qui décide de ce
    // qu'elle sacrifie quand la place manque, pas le rognage du `Paragraph`.
    frame.render_widget(Paragraph::new(app.status_line(areas[1].width)), areas[1]);
}

/// Une ligne : deux pictogrammes colorés, à largeur fixe, puis le texte.
fn item(line: ListRow) -> ListItem<'static> {
    let mut text_style = Style::default();
    if line.dim {
        text_style = text_style.add_modifier(Modifier::DIM);
    }
    ListItem::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            line.checks.symbol.to_string(),
            Style::default().fg(color(line.checks.tone)),
        ),
        Span::raw(" "),
        Span::styled(
            line.review.symbol.to_string(),
            Style::default().fg(color(line.review.tone)),
        ),
        Span::raw("  "),
        Span::styled(line.text, text_style),
    ]))
}
