//! Dessin de la fenêtre de confirmation de fusion.
//!
//! Aucune décision : les lignes sont composées par `app`. Ici, seulement la
//! taille, le centrage, l'effacement du fond et le cadre.

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::MergeRender;

/// Marge intérieure de chaque côté, en plus des deux colonnes de bordure.
const MARGE: u16 = 2;

pub fn draw(frame: &mut Frame, area: Rect, rendu: &MergeRender) {
    let contenu = rendu
        .lines
        .iter()
        .map(|ligne| ligne.chars().count())
        .max()
        .unwrap_or(0);
    let contenu = contenu.max(rendu.title.chars().count());

    // Deux colonnes de bordure, deux marges de chaque côté. La fenêtre ne
    // dépasse jamais l'écran, même si un message de GitHub est très long.
    let largeur = u16::try_from(contenu + 2 + 2 * MARGE as usize)
        .unwrap_or(u16::MAX)
        .min(area.width);
    let hauteur = u16::try_from(rendu.lines.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height);

    let zone = Rect {
        x: area.x + (area.width.saturating_sub(largeur)) / 2,
        y: area.y + (area.height.saturating_sub(hauteur)) / 2,
        width: largeur,
        height: hauteur,
    };

    // `Clear` d'abord : sans lui, la liste resterait visible sous la fenêtre.
    frame.render_widget(Clear, zone);

    let cadre = Block::default()
        .borders(Borders::ALL)
        .title(rendu.title.clone());
    let interieur = cadre.inner(zone);
    frame.render_widget(cadre, zone);

    let texte = rendu.lines.join("\n");
    frame.render_widget(
        Paragraph::new(texte),
        Rect {
            x: interieur.x + MARGE.min(interieur.width),
            y: interieur.y,
            width: interieur.width.saturating_sub(2 * MARGE),
            height: interieur.height,
        },
    );
}
