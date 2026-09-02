//! Dessin de la liste des pull requests et de la barre d'état.
//!
//! Aucune décision : les pictogrammes, les colonnes, la troncature et les
//! messages sont composés par `app`. Ici, seulement la mise en page, la
//! couleur, et le curseur de sélection.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, ListRender, ListRow, Tone};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let cadre = Block::default()
        .borders(Borders::ALL)
        .title(" owl — pull requests ");
    // La largeur utile est celle de l'intérieur du cadre : c'est elle que
    // `app` doit connaître pour décider de la troncature.
    let interieur = cadre.inner(zones[0]);
    frame.render_widget(cadre, zones[0]);

    match app.list_render(interieur.width) {
        ListRender::Rows(lignes) => {
            let items: Vec<ListItem> = lignes.into_iter().map(item).collect();
            // L'état de sélection est reconstruit à chaque dessin depuis
            // `app.selected` : c'est lui qui fait défiler la liste quand elle
            // dépasse la hauteur, et `ui` ne retient rien entre deux dessins.
            let mut etat = ListState::default().with_selected(Some(app.selected));
            let liste =
                List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(liste, interieur, &mut etat);
        }
        ListRender::Empty(lignes) => {
            let texte: Vec<Line> = lignes.into_iter().map(Line::from).collect();
            frame.render_widget(Paragraph::new(texte), interieur);
        }
        ListRender::TooNarrow(message) => {
            frame.render_widget(Paragraph::new(message), interieur);
        }
    }

    frame.render_widget(Paragraph::new(app.status_line()), zones[1]);
}

/// Une ligne : deux pictogrammes colorés, à largeur fixe, puis le texte.
fn item(ligne: ListRow) -> ListItem<'static> {
    let mut style_texte = Style::default();
    if ligne.dim {
        style_texte = style_texte.add_modifier(Modifier::DIM);
    }
    ListItem::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            ligne.checks.symbol.to_string(),
            Style::default().fg(couleur(ligne.checks.tone)),
        ),
        Span::raw(" "),
        Span::styled(
            ligne.review.symbol.to_string(),
            Style::default().fg(couleur(ligne.review.tone)),
        ),
        Span::raw("  "),
        Span::styled(ligne.text, style_texte),
    ]))
}

fn couleur(ton: Tone) -> Color {
    match ton {
        Tone::Vert => Color::Green,
        Tone::Rouge => Color::Red,
        Tone::Jaune => Color::Yellow,
        Tone::Gris => Color::DarkGray,
    }
}
