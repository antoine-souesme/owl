//! Composition de l'affichage : pictogrammes, colonnes, troncature, messages.
//!
//! Tout ce qui se décide avant de dessiner est ici, et rien de ce qui est ici
//! ne touche au terminal. `ui` reçoit des chaînes prêtes et des tons, et
//! n'ajoute que la mise en page et la couleur.

use crate::app::{App, MergeDialogState, View};
use crate::filter::Filter;
use crate::model::{ChecksState, MergeMethod, MergeableState, PrDetail, PrSummary, ReviewState};

/// Couleur logique d'un élément. `ui` la traduit en couleur de terminal ;
/// le sens — vert pour « ça passe » — est décidé ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Green,
    Red,
    Yellow,
    Gray,
}

/// Un pictogramme et son ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub symbol: char,
    pub tone: Tone,
}

/// Une ligne de liste, prête à dessiner.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    pub checks: Glyph,
    pub review: Glyph,
    /// Dépôt, numéro et titre : colonnes déjà alignées, titre déjà tronqué,
    /// marques du brouillon et du conflit déjà posées.
    pub text: String,
    /// Ligne grisée, parce que la pull request est un brouillon.
    pub dim: bool,
}

/// Ce qu'il y a à dessiner à la place de la liste.
#[derive(Debug, Clone, PartialEq)]
pub enum ListRender {
    Rows(Vec<ListRow>),
    /// Aucune pull request : le message, puis le rappel des filtres actifs.
    Empty(Vec<String>),
    /// Terminal trop étroit pour le dépôt et le numéro, qui ne se tronquent
    /// jamais : mieux vaut le dire qu'afficher une bouillie.
    TooNarrow(String),
}

/// Largeur fixe des deux colonnes de pictogrammes, séparateur compris :
/// une espace, un pictogramme, une espace, un pictogramme, deux espaces.
const GLYPHS: usize = 6;

/// Espacement entre deux colonnes de texte.
const GAP: usize = 2;

const TOO_NARROW: &str = "Élargis le terminal : le dépôt et le numéro n'y tiennent pas.";

const EMPTY_LIST: &str = "Aucune pull request";

impl App {
    /// Compose la liste pour une largeur donnée, celle de l'intérieur du cadre.
    ///
    /// La largeur entre ici parce que la troncature en dépend, et qu'elle est
    /// une décision : `ui` ne coupe jamais un texte lui-même.
    pub fn list_render(&self, width: u16) -> ListRender {
        if self.prs.is_empty() {
            return ListRender::Empty(vec![
                EMPTY_LIST.to_string(),
                format!("Filtres actifs : {}", self.active_filters()),
            ]);
        }

        let width = width as usize;
        let repo_column = self
            .prs
            .iter()
            .map(|pr| pr.key.repo.chars().count())
            .max()
            .unwrap_or(0);
        let number_column = self
            .prs
            .iter()
            .map(|pr| number(pr).chars().count())
            .max()
            .unwrap_or(0);

        let minimum = GLYPHS + repo_column + GAP + number_column;
        if width < minimum {
            return ListRender::TooNarrow(TOO_NARROW.to_string());
        }
        let title_width = width.saturating_sub(minimum + GAP);

        ListRender::Rows(
            self.prs
                .iter()
                .map(|pr| ListRow {
                    checks: checks_glyph(pr.checks),
                    review: review_glyph(pr.review),
                    text: row_cells(pr, repo_column, number_column, title_width),
                    dim: pr.is_draft,
                })
                .collect(),
        )
    }

    /// Rappel des filtres actifs, pour la liste vide.
    fn active_filters(&self) -> String {
        self.filters
            .iter()
            .map(Filter::fragment)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn number(pr: &PrSummary) -> String {
    format!("#{}", pr.key.number)
}

/// Dépôt, numéro et titre, en colonnes alignées.
fn row_cells(
    pr: &PrSummary,
    repo_column: usize,
    number_column: usize,
    title_width: usize,
) -> String {
    let mut line = format!(
        "{:<repo_column$}  {:<number_column$}",
        pr.key.repo,
        number(pr)
    );
    if title_width > 0 {
        line.push_str("  ");
        line.push_str(&truncate(&displayed_title(pr), title_width));
    }
    // La dernière colonne ne porte pas de remplissage inutile.
    line.trim_end().to_string()
}

/// Titre avec ses marques : le brouillon qualifie la pull request, le conflit
/// qualifie sa fusion. Un état de fusion inconnu n'affiche rien, GitHub étant
/// peut-être encore en train de le calculer.
fn displayed_title(pr: &PrSummary) -> String {
    let mut title = String::new();
    if pr.is_draft {
        title.push_str("[brouillon] ");
    }
    if pr.mergeable == MergeableState::Conflicting {
        title.push_str("⚠ ");
    }
    title.push_str(&pr.title);
    title
}

/// Coupe à la largeur donnée, en marquant la coupe. La mesure se fait en
/// caractères : compter les colonnes réellement occupées demanderait une
/// dépendance de plus.
pub(super) fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return text.chars().take(width).collect();
    }
    let mut cut: String = text.chars().take(width - 1).collect();
    cut.push('…');
    cut
}

fn checks_glyph(state: ChecksState) -> Glyph {
    match state {
        ChecksState::Success => Glyph {
            symbol: '✓',
            tone: Tone::Green,
        },
        ChecksState::Failure => Glyph {
            symbol: '✗',
            tone: Tone::Red,
        },
        ChecksState::Pending => Glyph {
            symbol: '○',
            tone: Tone::Yellow,
        },
        ChecksState::None => Glyph {
            symbol: '·',
            tone: Tone::Gray,
        },
    }
}

fn review_glyph(state: ReviewState) -> Glyph {
    match state {
        ReviewState::Approved => Glyph {
            symbol: '✓',
            tone: Tone::Green,
        },
        ReviewState::ChangesRequested => Glyph {
            symbol: '✗',
            tone: Tone::Red,
        },
        ReviewState::ReviewRequired => Glyph {
            symbol: '●',
            tone: Tone::Yellow,
        },
        ReviewState::None => Glyph {
            symbol: '·',
            tone: Tone::Gray,
        },
    }
}

/// Une ligne de la vue détail, prête à dessiner. `tone` absent : couleur par
/// défaut du terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailLine {
    pub text: String,
    pub tone: Option<Tone>,
}

impl DetailLine {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: None,
        }
    }

    fn toned(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone: Some(tone),
        }
    }
}

const LOADING_DETAIL: &str = "Chargement du détail…";
const NO_DESCRIPTION: &str = "(aucune description)";

impl App {
    /// Nombre de lignes du détail. La largeur ne change que leur longueur,
    /// jamais leur nombre : le défilement peut donc se borner sans elle.
    pub(crate) fn detail_line_count(&self) -> usize {
        self.detail_lines(u16::MAX).len()
    }

    /// Compose la vue détail : une seule zone qui défile, pas un ensemble de
    /// panneaux. Tant que la requête n'a pas répondu, l'en-tête vient du
    /// résumé déjà en mémoire et le reste annonce le chargement.
    pub fn detail_lines(&self, width: u16) -> Vec<DetailLine> {
        let View::Detail { key, .. } = &self.view else {
            return Vec::new();
        };
        let cache = self.details.get(key);
        let Some(summary) = self.displayed_summary(key) else {
            return Vec::new();
        };

        let mut lines = vec![
            DetailLine::plain(format!(
                "{}  #{}  {}",
                summary.key.repo, summary.key.number, summary.title
            )),
            DetailLine::plain(format!("par {}", summary.author)),
        ];

        match cache {
            None => lines.push(DetailLine::plain(LOADING_DETAIL)),
            Some(cache) => {
                lines.extend(detail_body(
                    &cache.detail,
                    &cache.loaded_at.format("%H:%M").to_string(),
                ));
            }
        }

        // La troncature est faite en dernier, sur toutes les lignes à la fois :
        // aucune n'a le droit de dépasser la zone.
        let width = width as usize;
        lines
            .into_iter()
            .map(|line| DetailLine {
                text: truncate(&line.text, width),
                tone: line.tone,
            })
            .collect()
    }
}

/// Corps du détail, dans l'ordre de la spec : branches, états en clair,
/// description, vérifications, échanges, fichiers.
fn detail_body(detail: &PrDetail, time: &str) -> Vec<DetailLine> {
    let mut lines = vec![
        DetailLine::plain(format!("de {} vers {}", detail.head_ref, detail.base_ref)),
        DetailLine::toned(
            checks_label(detail.summary.checks),
            checks_glyph(detail.summary.checks).tone,
        ),
        DetailLine::toned(
            review_label(detail.summary.review),
            review_glyph(detail.summary.review).tone,
        ),
        DetailLine::plain(mergeable_label(detail.summary.mergeable)),
        DetailLine::plain(String::new()),
    ];

    if detail.body.trim().is_empty() {
        lines.push(DetailLine::plain(NO_DESCRIPTION));
    } else {
        lines.extend(detail.body.lines().map(DetailLine::plain));
    }
    lines.push(DetailLine::plain(String::new()));

    lines.push(DetailLine::plain(format!(
        "Vérifications ({})",
        detail.checks.len()
    )));
    for check in &detail.checks {
        let glyph = checks_glyph(check.state);
        lines.push(DetailLine::toned(
            format!("  {} {}", glyph.symbol, check.name),
            glyph.tone,
        ));
    }
    lines.push(DetailLine::plain(String::new()));

    lines.push(DetailLine::plain("Relectures et commentaires"));
    lines.extend(conversation(detail));
    lines.push(DetailLine::plain(String::new()));

    lines.push(DetailLine::plain(format!(
        "Fichiers modifiés ({}) · +{} -{}",
        detail.files.len(),
        detail.additions,
        detail.deletions
    )));
    for file in &detail.files {
        lines.push(DetailLine::plain(format!(
            "  {}  +{} -{}",
            file.path, file.additions, file.deletions
        )));
    }

    lines.push(DetailLine::plain(String::new()));
    lines.push(DetailLine::toned(
        format!("Détail chargé à {time}"),
        Tone::Gray,
    ));
    lines
}

/// Relectures et commentaires fondus dans un seul fil chronologique : c'est
/// l'ordre dans lequel la conversation a eu lieu.
fn conversation(detail: &PrDetail) -> Vec<DetailLine> {
    let mut thread: Vec<(chrono::DateTime<chrono::Utc>, String)> = Vec::new();
    for review in &detail.reviews {
        thread.push((
            review.submitted_at,
            format!(
                "  {} · {} · {}",
                review.author,
                review_label(review.state),
                review.body.replace('\n', " ")
            ),
        ));
    }
    for comment in &detail.comments {
        thread.push((
            comment.created_at,
            format!("  {} · {}", comment.author, comment.body.replace('\n', " ")),
        ));
    }
    thread.sort_by_key(|(at, _)| *at);
    thread
        .into_iter()
        .map(|(_, text)| DetailLine::plain(text))
        .collect()
}

fn checks_label(state: ChecksState) -> &'static str {
    match state {
        ChecksState::Success => "toutes les vérifications passent",
        ChecksState::Failure => "au moins une vérification échoue",
        ChecksState::Pending => "vérifications en cours",
        ChecksState::None => "aucune vérification",
    }
}

fn review_label(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Approved => "approuvée",
        ReviewState::ChangesRequested => "changements demandés",
        ReviewState::ReviewRequired => "relecture attendue",
        ReviewState::None => "rien à signaler",
    }
}

fn mergeable_label(state: MergeableState) -> &'static str {
    match state {
        MergeableState::Mergeable => "fusion possible",
        MergeableState::Conflicting => "conflits à résoudre",
        // Une attente, pas un blocage : GitHub calcule ce champ à la demande.
        MergeableState::Unknown => "état de fusion en cours de calcul",
    }
}

/// La fenêtre de fusion, prête à dessiner : un titre de cadre et des lignes
/// déjà écrites, chevron de sélection compris. `ui` ne compose rien.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeRender {
    pub title: String,
    pub lines: Vec<String>,
}

const MERGE_TITLE: &str = " Fusionner ";
const HELP_CHOOSING: &str = "Entrée pour confirmer · Échap pour annuler";
const HELP_FAILED: &str = "Entrée pour réessayer · Échap pour fermer";
const MERGING: &str = "Fusion en cours…";

/// Largeur de contenu au-delà de laquelle une ligne se replie plutôt que de
/// s'étaler sur tout le terminal : une fenêtre modale reste lisible, elle ne
/// prend pas toute la largeur disponible sous prétexte qu'elle le pourrait.
const MAX_CONTENT_WIDTH: usize = 60;

/// Ce que `ui::merge` retire de la largeur disponible pour poser la fenêtre :
/// les deux colonnes de bordure et, de chaque côté, la marge intérieure
/// (`MARGIN` dans `ui/merge.rs`, à tenir à jour avec cette valeur).
const OUTSIDE_CONTENT: usize = 2 + 2 * 2;

/// Libellé d'une méthode dans la liste de choix.
fn label(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Squash => "Écraser les commits (squash)",
        MergeMethod::Rebase => "Rebaser (rebase)",
        MergeMethod::Merge => "Créer un commit de fusion (merge)",
    }
}

/// Libellé d'une méthode au fil d'une phrase, sans capitale ni parenthèse.
fn short_method_label(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Squash => "écraser les commits",
        MergeMethod::Rebase => "rebaser",
        MergeMethod::Merge => "créer un commit de fusion",
    }
}

impl App {
    /// Compose la fenêtre de fusion, s'il y en a une, pour la largeur
    /// disponible donnée — comme `status_line(width)`.
    ///
    /// Chaque ligne est repliée ici, contre une largeur de contenu bornée à
    /// la fois par l'espace disponible et par `MAX_CONTENT_WIDTH` : `ui` ne
    /// mesure plus que ce qu'il reçoit, il ne coupe et ne replie rien. Le
    /// message d'erreur de GitHub n'est ainsi jamais tronqué, même très long.
    pub fn merge_render(&self, width: u16) -> Option<MergeRender> {
        let dialog = self.merge.as_ref()?;
        let content_width = (width as usize)
            .saturating_sub(OUTSIDE_CONTENT)
            .clamp(1, MAX_CONTENT_WIDTH);

        let mut lines = vec![
            format!("{} #{}", dialog.key.repo, dialog.key.number),
            dialog.title.clone(),
            String::new(),
        ];

        match &dialog.state {
            MergeDialogState::Choosing => {
                // Une seule méthode autorisée : rien à choisir, on le dit.
                if let [only_one] = dialog.methods.as_slice() {
                    lines.push(format!(
                        "Méthode : {} (imposé par le dépôt)",
                        short_method_label(*only_one)
                    ));
                } else {
                    lines.push("Méthode :".to_string());
                    for (index, method) in dialog.methods.iter().enumerate() {
                        let caret = if index == dialog.selected { ">" } else { " " };
                        lines.push(format!("  {caret} {}", label(*method)));
                    }
                }
                lines.push(String::new());
                lines.push(HELP_CHOOSING.to_string());
            }
            MergeDialogState::Submitting => {
                if let Some(method) = dialog.method() {
                    lines.push(format!("Méthode : {}", short_method_label(method)));
                }
                lines.push(String::new());
                lines.push(MERGING.to_string());
            }
            MergeDialogState::Failed(message) => {
                lines.push(message.clone());
                lines.push(String::new());
                lines.push(HELP_FAILED.to_string());
            }
        }

        let lines = lines
            .iter()
            .flat_map(|line| wrap(line, content_width))
            .collect();

        Some(MergeRender {
            title: MERGE_TITLE.to_string(),
            lines: lines,
        })
    }
}

/// Replie un texte contre une largeur donnée, en coupant sur les limites de
/// mots quand c'est possible. Une ligne qui tient déjà n'est pas touchée : les
/// libellés courts (dépôt, titre, aides) ressortent intacts. Un mot lui-même
/// plus large que la largeur est coupé faute de mieux, mais rien n'est jamais
/// perdu — c'est ce qui garantit que le message de GitHub reste entier.
fn wrap(text: &str, width: usize) -> Vec<String> {
    if text.chars().count() <= width {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();

    for word in text.split(' ') {
        let length_with_word = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };

        if !current.is_empty() && length_with_word > width {
            result.push(std::mem::take(&mut current));
        }

        if word.chars().count() > width {
            if !current.is_empty() {
                result.push(std::mem::take(&mut current));
            }
            let mut rest = word;
            while rest.chars().count() > width {
                let cut: String = rest.chars().take(width).collect();
                rest = &rest[cut.len()..];
                result.push(cut);
            }
            current = rest.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }

    result.push(current);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app::tests::detail;
    use crate::app::tests::{app_with, page, pr, pr_in, pr_with_rules};
    use crate::app::{Command, Event, Key, MergeDialogState, View};
    use crate::config::Config;
    use crate::model::RepoMergeRules;

    /// Largeur confortable : aucun titre n'y est tronqué.
    const LARGE: u16 = 120;

    fn lines(app: &crate::app::App, width: u16) -> Vec<ListRow> {
        match app.list_render(width) {
            ListRender::Rows(lines) => lines,
            other => panic!("rendu inattendu : {other:?}"),
        }
    }

    /// Détail ouvert sur la PR donnée, réponse livrée.
    fn app_in_detail(number: u32) -> crate::app::App {
        let mut app = app_with(vec![pr(number)]);
        let generation = match &app.handle(Event::Key(Key::Right))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(number).key,
            result: Ok(detail(number)),
        });
        app
    }

    fn texts(app: &crate::app::App) -> Vec<String> {
        app.detail_lines(LARGE)
            .into_iter()
            .map(|line| line.text)
            .collect()
    }

    #[test]
    fn the_header_shows_before_the_response_and_the_rest_says_loading() {
        let mut app = app_with(vec![pr(142)]);
        app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));

        let texts = texts(&app);
        assert!(
            texts[0].contains("moi/depot") && texts[0].contains("#142"),
            "l'en-tête vient de PrSummary, déjà en mémoire : {texts:?}"
        );
        assert!(texts[0].contains("Titre 142"), "{texts:?}");
        assert!(
            texts.iter().any(|line| line.contains("Chargement")),
            "{texts:?}"
        );
    }

    #[test]
    fn the_detail_gives_the_states_in_plain_words() {
        let texts = texts(&app_in_detail(1)).join("\n");
        assert!(texts.contains("de ma-branche vers develop"), "{texts}");
        // « par moi » et pas « moi » : « moi » figure déjà dans le dépôt de
        // la ligne d'en-tête, l'assertion tiendrait sans ligne d'auteur.
        assert!(texts.contains("par moi"), "l'auteur : {texts}");
        assert!(
            texts.contains("toutes les vérifications passent"),
            "les mêmes états que la liste, en clair : {texts}"
        );
        assert!(texts.contains("approuvée"), "{texts}");
    }

    #[test]
    fn the_detail_lists_the_description_the_checks_the_conversation_and_the_files() {
        let texts = texts(&app_in_detail(1)).join("\n");
        assert!(texts.contains("Première ligne."), "{texts}");
        assert!(texts.contains("Seconde ligne."), "{texts}");
        assert!(
            texts.contains("tests"),
            "une vérification par ligne : {texts}"
        );
        assert!(texts.contains("collegue"), "une relecture : {texts}");
        assert!(texts.contains("Rebasé."), "un commentaire : {texts}");
        assert!(
            texts.contains("src/app/mod.rs") && texts.contains("+12") && texts.contains("-3"),
            "les fichiers et leurs compteurs : {texts}"
        );
    }

    #[test]
    fn the_reviews_and_the_comments_are_in_chronological_order() {
        let texts = texts(&app_in_detail(1)).join("\n");
        let review = texts.find("collegue").expect("la relecture de 10:00");
        let comment = texts.find("Rebasé.").expect("le commentaire de 11:00");
        assert!(review < comment, "{texts}");
    }

    #[test]
    fn the_detail_carries_the_time_it_was_loaded() {
        let app = app_in_detail(1);
        let time = app
            .details
            .values()
            .next()
            .expect("un détail en cache")
            .loaded_at
            .format("%H:%M")
            .to_string();
        assert!(
            texts(&app).iter().any(|line| line.contains(&time)),
            "le détail peut être périmé : autant dire quand il a été lu"
        );
    }

    #[test]
    fn a_detail_line_too_long_is_truncated() {
        let app = app_in_detail(1);
        for line in app.detail_lines(40) {
            assert!(line.text.chars().count() <= 40, "ligne = {}", line.text);
        }
    }

    #[test]
    fn a_cached_detail_stays_displayable_when_the_pr_leaves_the_list() {
        let mut app = app_in_detail(1);

        // La PR quitte la liste (fusionnée, filtrée...) pendant que le détail reste ouvert.
        let list_generation_id = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: list_generation_id,
            result: Ok(page(vec![])),
        });

        let texts = texts(&app);
        assert!(
            texts[0].contains("Titre 1"),
            "l'en-tête vient du résumé porté par le détail en cache : {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|line| line.contains("de ma-branche vers develop")),
            "le corps reste composé depuis le cache : {texts:?}"
        );
    }

    #[test]
    fn a_row_carries_the_two_glyphs_then_the_repo_the_number_and_the_title() {
        let app = app_with(vec![pr(142)]);
        let line = lines(&app, LARGE).remove(0);
        assert_eq!(
            line.checks,
            Glyph {
                symbol: '✓',
                tone: Tone::Green
            }
        );
        assert_eq!(
            line.review,
            Glyph {
                symbol: '✓',
                tone: Tone::Green
            }
        );
        assert_eq!(line.text, "moi/depot  #142  Titre 142");
        assert!(!line.dim);
    }

    #[test]
    fn each_checks_state_has_its_glyph() {
        let cases = [
            (ChecksState::Success, '✓', Tone::Green),
            (ChecksState::Failure, '✗', Tone::Red),
            (ChecksState::Pending, '○', Tone::Yellow),
            (ChecksState::None, '·', Tone::Gray),
        ];
        for (state, symbol, tone) in cases {
            let app = app_with(vec![PrSummary {
                checks: state,
                ..pr(1)
            }]);
            assert_eq!(
                lines(&app, LARGE)[0].checks,
                Glyph {
                    symbol: symbol,
                    tone: tone
                },
                "état = {state:?}"
            );
        }
    }

    #[test]
    fn each_review_state_has_its_glyph() {
        let cases = [
            (ReviewState::Approved, '✓', Tone::Green),
            (ReviewState::ChangesRequested, '✗', Tone::Red),
            (ReviewState::ReviewRequired, '●', Tone::Yellow),
            (ReviewState::None, '·', Tone::Gray),
        ];
        for (state, symbol, tone) in cases {
            let app = app_with(vec![PrSummary {
                review: state,
                ..pr(1)
            }]);
            assert_eq!(
                lines(&app, LARGE)[0].review,
                Glyph {
                    symbol: symbol,
                    tone: tone
                },
                "état = {state:?}"
            );
        }
    }

    #[test]
    fn a_draft_is_prefixed_and_dimmed() {
        let app = app_with(vec![PrSummary {
            is_draft: true,
            ..pr(150)
        }]);
        let line = lines(&app, LARGE).remove(0);
        assert_eq!(line.text, "moi/depot  #150  [brouillon] Titre 150");
        assert!(line.dim, "la ligne d'un brouillon est grisée");
    }

    #[test]
    fn a_merge_conflict_is_flagged_before_the_title() {
        let app = app_with(vec![PrSummary {
            mergeable: MergeableState::Conflicting,
            ..pr(31)
        }]);
        assert_eq!(lines(&app, LARGE)[0].text, "moi/depot  #31  ⚠ Titre 31");
    }

    #[test]
    fn an_unknown_merge_state_shows_nothing() {
        let app = app_with(vec![PrSummary {
            mergeable: MergeableState::Unknown,
            ..pr(31)
        }]);
        assert_eq!(
            lines(&app, LARGE)[0].text,
            "moi/depot  #31  Titre 31",
            "GitHub calcule peut-être encore : ne rien annoncer"
        );
    }

    #[test]
    fn a_conflicting_draft_carries_both_marks() {
        let app = app_with(vec![PrSummary {
            is_draft: true,
            mergeable: MergeableState::Conflicting,
            ..pr(7)
        }]);
        assert_eq!(
            lines(&app, LARGE)[0].text,
            "moi/depot  #7  [brouillon] ⚠ Titre 7"
        );
    }

    #[test]
    fn the_repos_and_the_numbers_line_up() {
        let app = app_with(vec![
            pr_in("moi/depot", 7),
            pr_in("moi/un-depot-plus-long", 150),
        ]);
        let lines = lines(&app, LARGE);
        let column = |line: &ListRow| line.text.find("  #").expect("colonne du numéro");
        assert_eq!(
            column(&lines[0]),
            column(&lines[1]),
            "les numéros commencent à la même colonne"
        );
        let title = |line: &ListRow| line.text.find("Titre").expect("colonne du titre");
        assert_eq!(title(&lines[0]), title(&lines[1]), "les titres aussi");
    }

    #[test]
    fn the_title_is_truncated_to_the_available_width() {
        let app = app_with(vec![PrSummary {
            title: "Un titre beaucoup trop long pour la fenêtre".to_string(),
            ..pr(1)
        }]);
        // 30 colonnes moins les 6 des pictogrammes, que `ui` ajoute lui-même.
        let line = lines(&app, 30).remove(0);
        assert_eq!(line.text.chars().count(), 24);
        assert!(line.text.starts_with("moi/depot  #1  "), "{}", line.text);
        assert!(line.text.ends_with('…'), "{}", line.text);
    }

    #[test]
    fn the_repo_and_the_number_are_never_truncated() {
        // Juste de quoi tenir les pictogrammes, le dépôt et le numéro.
        let app = app_with(vec![pr(142)]);
        let line = lines(&app, 6 + 9 + 2 + 4).remove(0);
        assert_eq!(
            line.text, "moi/depot  #142",
            "pas de titre, mais tout le reste"
        );
    }

    #[test]
    fn a_window_too_narrow_asks_to_be_widened() {
        let app = app_with(vec![pr(142)]);
        match app.list_render(10) {
            ListRender::TooNarrow(message) => {
                assert!(message.contains("Élargis"), "message = {message}")
            }
            other => panic!("rendu inattendu : {other:?}"),
        }
    }

    #[test]
    fn an_empty_list_recalls_the_active_filters() {
        let app = app_with(vec![]);
        match app.list_render(LARGE) {
            ListRender::Empty(lines) => {
                assert_eq!(lines[0], "Aucune pull request");
                assert!(
                    lines[1].contains("author:@me") && lines[1].contains("is:open"),
                    "un filtre trop restrictif ressemble sinon à une panne : {}",
                    lines[1]
                );
            }
            other => panic!("rendu inattendu : {other:?}"),
        }
    }

    #[test]
    fn an_empty_list_with_unusual_filters_recalls_them_too() {
        let settings = Config {
            filters: vec!["org:acme".to_string(), "involves:@me".to_string()],
            ..Config::default()
        };
        let app = crate::app::App::new(settings);
        match app.list_render(LARGE) {
            ListRender::Empty(lines) => {
                assert!(lines[1].contains("org:acme"), "{}", lines[1]);
                assert!(lines[1].contains("involves:@me"), "{}", lines[1]);
            }
            other => panic!("rendu inattendu : {other:?}"),
        }
    }

    fn all_allowed() -> RepoMergeRules {
        RepoMergeRules {
            squash: true,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        }
    }

    #[test]
    fn with_no_dialog_open_there_is_nothing_to_draw() {
        let app = app_with(vec![pr_with_rules(1, all_allowed())]);
        assert!(app.merge_render(LARGE).is_none());
    }

    #[test]
    fn the_dialog_shows_the_repo_the_title_and_the_allowed_methods() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");

        assert_eq!(render.title, " Fusionner ");
        assert_eq!(render.lines[0], "moi/depot #142");
        assert_eq!(render.lines[1], "Titre 142");
        assert!(render.lines.contains(&"Méthode :".to_string()));
        assert!(render
            .lines
            .contains(&"  > Écraser les commits (squash)".to_string()));
        assert!(render.lines.contains(&"    Rebaser (rebase)".to_string()));
        assert!(render
            .lines
            .contains(&"    Créer un commit de fusion (merge)".to_string()));
        assert!(render
            .lines
            .contains(&"Entrée pour confirmer · Échap pour annuler".to_string()));
    }

    #[test]
    fn a_single_allowed_method_replaces_the_list_with_one_line() {
        let rules = RepoMergeRules {
            squash: true,
            merge: false,
            rebase: false,
            delete_branch_on_merge: true,
        };
        let mut app = app_with(vec![pr_with_rules(142, rules)]);
        app.handle(Event::Key(Key::Char('m')));
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");

        assert!(render
            .lines
            .contains(&"Méthode : écraser les commits (imposé par le dépôt)".to_string()));
        assert!(!render.lines.iter().any(|line| line.contains('>')));
    }

    #[test]
    fn the_dialog_announces_the_merge_in_progress_without_closing() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        app.handle(Event::Key(Key::Enter));
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit rester ouverte");

        assert!(render.lines.contains(&"Fusion en cours…".to_string()));
        assert!(!render
            .lines
            .iter()
            .any(|line| line.contains("Échap pour annuler")));
    }

    #[test]
    fn a_failure_shows_the_github_message_verbatim() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        if let Some(dialog) = app.merge.as_mut() {
            dialog.state = MergeDialogState::Failed("Base branch was modified.".to_string());
        }
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");

        assert!(render
            .lines
            .contains(&"Base branch was modified.".to_string()));
        assert!(render
            .lines
            .contains(&"Entrée pour réessayer · Échap pour fermer".to_string()));
    }

    #[test]
    fn an_error_message_too_long_is_wrapped_rather_than_truncated() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        let message = "Required status check \"tests\" is expected. \
             · At least 1 approving review is required by reviewers with write access."
            .to_string();
        if let Some(dialog) = app.merge.as_mut() {
            dialog.state = MergeDialogState::Failed(message.clone());
        }
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");

        // Aucune ligne ne dépasse la largeur maximale de contenu de la
        // fenêtre : le message est reconstitué en le repliant.
        for line in &render.lines {
            assert!(
                line.chars().count() <= 60,
                "ligne trop longue, non repliée : {line}"
            );
        }

        // Rien n'est perdu : les morceaux du message, mis bout à bout avec
        // une espace, redonnent le message d'origine.
        let start_index = render
            .lines
            .iter()
            .position(|line| line.starts_with("Required"))
            .expect("le message doit apparaître : {render:?}");
        let end_index = render
            .lines
            .iter()
            .position(|line| line.ends_with("write access."))
            .expect("la fin du message doit apparaître");
        let reconstitue = render.lines[start_index..=end_index].join(" ");
        assert_eq!(reconstitue, message);
    }
}
