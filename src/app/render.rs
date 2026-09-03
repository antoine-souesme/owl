//! Composition de l'affichage : pictogrammes, colonnes, troncature, messages.
//!
//! Tout ce qui se décide avant de dessiner est ici, et rien de ce qui est ici
//! ne touche au terminal. `ui` reçoit des chaînes prêtes et des tons, et
//! n'ajoute que la mise en page et la couleur.

use chrono::{DateTime, Utc};

use crate::app::{App, MergeDialogState, View};
use crate::filter::Filter;
use crate::model::{ChecksState, MergeMethod, MergeableState, PrDetail, PrSummary, ReviewState};

/// Couleur logique d'un élément. `ui` la traduit en couleur de terminal ;
/// le sens — vert pour « ça passe », cyan pour le dépôt — est décidé ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Green,
    Red,
    Yellow,
    Gray,
    Cyan,
    Blue,
}

/// Un pictogramme et son ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub symbol: char,
    pub tone: Tone,
}

/// Un morceau de ligne et son ton. Le remplissage des colonnes est déjà
/// posé : `ui` met les morceaux bout à bout, sans rien mesurer.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub text: String,
    /// Ton absent : couleur par défaut du terminal.
    pub tone: Option<Tone>,
}

impl Cell {
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

/// Une ligne de liste, prête à dessiner.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    pub checks: Glyph,
    pub review: Glyph,
    /// Dépôt, numéro, âge, branche cible et titre : colonnes déjà alignées,
    /// titre déjà tronqué, marques du brouillon et du conflit déjà posées.
    pub cells: Vec<Cell>,
    /// Ligne grisée, parce que la pull request est un brouillon.
    pub dim: bool,
}

impl ListRow {
    /// Ligne entière, morceaux mis bout à bout. Sert aux tests : le dessin,
    /// lui, garde les morceaux pour les colorer un à un.
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.cells.iter().map(|cell| cell.text.as_str()).collect()
    }
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

/// Marqueur de la ligne sélectionnée. C'est le seul signe de la sélection :
/// inverser les couleurs de la ligne transformerait les tons des colonnes en
/// fonds colorés, et la ligne cesserait de se lire comme les autres.
pub const SELECTION_MARKER: &str = "→ ";

/// Largeur fixe de la tête de ligne : le marqueur de sélection, un
/// pictogramme, une espace, un pictogramme, deux espaces. `ui` la pose,
/// `app` la déduit de la place disponible.
const GLYPHS: usize = 7;

/// Espacement entre deux colonnes de texte.
const GAP: usize = 2;

/// Séparateur entre le dépôt et le numéro : les deux se lisent ensemble, une
/// simple espace les confondrait.
const SEPARATOR: &str = " │ ";

/// Titres des cadres. Ce sont des messages : ils se décident ici, `ui` ne
/// fait que les poser sur la bordure.
pub const LIST_TITLE: &str = " Owl - Monitoring pull requests ";
pub const DETAIL_TITLE: &str = " Owl - Pull request details ";

const TOO_NARROW: &str = "Widen the terminal: the repository and the number do not fit.";
const EMPTY_LIST: &str = "No pull requests";

impl App {
    /// Compose la liste pour une largeur donnée, celle de l'intérieur du cadre.
    ///
    /// La largeur entre ici parce que la troncature en dépend, et qu'elle est
    /// une décision : `ui` ne coupe jamais un texte lui-même.
    pub fn list_render(&self, width: u16) -> ListRender {
        if self.prs.is_empty() {
            return ListRender::Empty(vec![
                EMPTY_LIST.to_string(),
                format!("Active filters: {}", self.active_filters()),
            ]);
        }

        let now = Utc::now();
        let width = width as usize;
        let column =
            |measure: &dyn Fn(&PrSummary) -> usize| self.prs.iter().map(measure).max().unwrap_or(0);
        let repo_column = column(&|pr: &PrSummary| pr.key.repo.chars().count());
        let number_column = column(&|pr: &PrSummary| number(pr).chars().count());
        let age_column = column(&|pr: &PrSummary| short_age(pr.updated_at, now).chars().count());
        let target_column = column(&|pr: &PrSummary| pr.base_ref.chars().count());

        let minimum = GLYPHS + repo_column + SEPARATOR.chars().count() + number_column;
        if width < minimum {
            return ListRender::TooNarrow(TOO_NARROW.to_string());
        }

        // Colonnes facultatives, prises dans cet ordre tant que la place
        // suit : l'âge, la cible, puis le titre avec ce qui reste. Une
        // colonne qui ne tient pas entière est abandonnée plutôt que coupée,
        // sauf le titre, qui est fait pour être tronqué.
        let mut rest = width - minimum;
        let age = fits(&mut rest, age_column);
        let target = fits(&mut rest, target_column);
        let title_width = rest.saturating_sub(GAP);

        ListRender::Rows(
            self.prs
                .iter()
                .map(|pr| ListRow {
                    checks: checks_glyph(pr.checks),
                    review: review_glyph(pr.review),
                    cells: row_cells(
                        pr,
                        now,
                        repo_column,
                        number_column,
                        age.then_some(age_column),
                        target.then_some(target_column),
                        title_width,
                    ),
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

/// Retient une colonne facultative si elle tient dans la place restante,
/// espacement compris, et retire alors ce qu'elle prend.
fn fits(rest: &mut usize, column: usize) -> bool {
    if column > 0 && *rest >= GAP + column {
        *rest -= GAP + column;
        true
    } else {
        false
    }
}

fn number(pr: &PrSummary) -> String {
    format!("#{}", pr.key.number)
}

/// Âge de la dernière mise à jour, en une poignée de caractères : minutes,
/// puis heures, puis jours. Une date dans le futur — horloges désaccordées —
/// donne zéro plutôt qu'un nombre négatif.
pub(crate) fn short_age(updated_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (now - updated_at).num_minutes().max(0);
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    format!("{}d", hours / 24)
}

/// Dépôt, numéro, âge, branche cible et titre, en colonnes alignées et
/// teintées. La dernière colonne ne porte pas de remplissage inutile.
fn row_cells(
    pr: &PrSummary,
    now: DateTime<Utc>,
    repo_column: usize,
    number_column: usize,
    age_column: Option<usize>,
    target_column: Option<usize>,
    title_width: usize,
) -> Vec<Cell> {
    let mut cells = vec![
        Cell::toned(format!("{:<repo_column$}", pr.key.repo), Tone::Cyan),
        Cell::toned(SEPARATOR, Tone::Gray),
        Cell::toned(format!("{:<number_column$}", number(pr)), Tone::Gray),
    ];
    if let Some(largeur) = age_column {
        cells.push(Cell::plain(" ".repeat(GAP)));
        cells.push(Cell::toned(
            format!("{:<largeur$}", short_age(pr.updated_at, now)),
            Tone::Gray,
        ));
    }
    if let Some(largeur) = target_column {
        cells.push(Cell::plain(" ".repeat(GAP)));
        cells.push(Cell::toned(
            format!("{:<largeur$}", pr.base_ref),
            Tone::Blue,
        ));
    }
    if title_width > 0 {
        cells.push(Cell::plain(" ".repeat(GAP)));
        cells.push(Cell::plain(truncate(&displayed_title(pr), title_width)));
    }

    // Le remplissage de la dernière colonne ne sert à rien : il allongerait
    // la ligne sans rien montrer.
    if let Some(last) = cells.last_mut() {
        last.text = last.text.trim_end().to_string();
    }
    cells.retain(|cell| !cell.text.is_empty());
    cells
}

/// Titre avec ses marques : le brouillon qualifie la pull request, le conflit
/// qualifie sa fusion. Un état de fusion inconnu n'affiche rien, GitHub étant
/// peut-être encore en train de le calculer.
fn displayed_title(pr: &PrSummary) -> String {
    let mut title = String::new();
    if pr.is_draft {
        title.push_str("[draft] ");
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

const LOADING_DETAIL: &str = "Loading details…";
const NO_DESCRIPTION: &str = "(no description)";

/// Largeur maximale de l'en-tête encadré : au-delà, un cadre qui suit toute
/// la largeur d'un terminal large sépare moins qu'il n'encombre.
const HEADER_WIDTH: usize = 72;

/// Retrait du contenu d'une section, sous son titre.
const INDENT: &str = "   ";

impl App {
    /// Nombre de lignes du détail. La largeur ne change que leur longueur,
    /// jamais leur nombre : le défilement peut donc se borner sans elle.
    pub(crate) fn detail_line_count(&self) -> usize {
        self.detail_lines(u16::MAX).len()
    }

    /// Compose la vue détail : une seule zone qui défile, pas un ensemble de
    /// panneaux. L'en-tête est encadré, le reste vient par sections titrées.
    /// Tant que la requête n'a pas répondu, l'en-tête vient du résumé déjà en
    /// mémoire et le reste annonce le chargement.
    pub fn detail_lines(&self, width: u16) -> Vec<DetailLine> {
        let View::Detail { key, .. } = &self.view else {
            return Vec::new();
        };
        let cache = self.details.get(key);
        let Some(summary) = self.displayed_summary(key) else {
            return Vec::new();
        };

        let mut lines = header_box(summary, (width as usize).min(HEADER_WIDTH));

        match cache {
            None => {
                lines.push(DetailLine::plain(String::new()));
                lines.push(DetailLine::plain(LOADING_DETAIL));
            }
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

/// En-tête encadré : dépôt et numéro, titre, puis auteur, âge et branche
/// cible. Le cadre est composé ici parce que sa largeur est une décision,
/// comme la troncature.
fn header_box(summary: &PrSummary, width: usize) -> Vec<DetailLine> {
    // Deux bordures et une espace de marge de chaque côté.
    let inner = width.saturating_sub(4);
    let row = |text: String| DetailLine::plain(format!("│ {:<inner$} │", truncate(&text, inner)));

    vec![
        DetailLine::toned(format!("┌{}┐", "─".repeat(inner + 2)), Tone::Gray),
        row(format!(
            "{}{SEPARATOR}#{}",
            summary.key.repo, summary.key.number
        )),
        row(displayed_title(summary)),
        // La branche cible n'est pas reprise ici : la section « Branches »
        // la donne, avec celle d'où part la pull request.
        row(format!(
            "by {} · {}",
            summary.author,
            short_age(summary.updated_at, Utc::now())
        )),
        DetailLine::toned(format!("└{}┘", "─".repeat(inner + 2)), Tone::Gray),
    ]
}

/// Titre de section : une ligne vide, puis le titre teinté. Les sections se
/// lisent ainsi comme des blocs et non comme une suite de lignes.
fn section(title: impl Into<String>) -> Vec<DetailLine> {
    vec![
        DetailLine::plain(String::new()),
        DetailLine::toned(format!(" {}", title.into()), Tone::Cyan),
    ]
}

/// Corps du détail, dans l'ordre de la spec : branches, états en clair,
/// description, vérifications, échanges, fichiers.
fn detail_body(detail: &PrDetail, time: &str) -> Vec<DetailLine> {
    let mut lines = section("Branches");
    lines.push(DetailLine::plain(format!(
        "{INDENT}{} -> {}",
        detail.head_ref, detail.summary.base_ref
    )));

    lines.extend(section("Status"));
    let checks = checks_glyph(detail.summary.checks);
    lines.push(DetailLine::toned(
        format!(
            "{INDENT}{} {}",
            checks.symbol,
            checks_label(detail.summary.checks)
        ),
        checks.tone,
    ));
    let review = review_glyph(detail.summary.review);
    lines.push(DetailLine::toned(
        format!(
            "{INDENT}{} {}",
            review.symbol,
            review_label(detail.summary.review)
        ),
        review.tone,
    ));
    let mergeable = mergeable_glyph(detail.summary.mergeable);
    lines.push(DetailLine::toned(
        format!(
            "{INDENT}{} {}",
            mergeable.symbol,
            mergeable_label(detail.summary.mergeable)
        ),
        mergeable.tone,
    ));

    lines.extend(section("Description"));
    if detail.body.trim().is_empty() {
        lines.push(DetailLine::plain(format!("{INDENT}{NO_DESCRIPTION}")));
    } else {
        lines.extend(
            detail
                .body
                .lines()
                .map(|line| DetailLine::plain(format!("{INDENT}{line}"))),
        );
    }

    lines.extend(section(format!("Checks ({})", detail.checks.len())));
    for check in &detail.checks {
        let glyph = checks_glyph(check.state);
        lines.push(DetailLine::toned(
            format!("{INDENT}{} {}", glyph.symbol, check.name),
            glyph.tone,
        ));
    }

    lines.extend(section("Reviews and comments"));
    lines.extend(conversation(detail));

    lines.extend(section(format!(
        "Files changed ({}) · +{} -{}",
        detail.files.len(),
        detail.additions,
        detail.deletions
    )));
    for file in &detail.files {
        lines.push(DetailLine::plain(format!(
            "{INDENT}{}  +{} -{}",
            file.path, file.additions, file.deletions
        )));
    }

    lines.push(DetailLine::plain(String::new()));
    lines.push(DetailLine::toned(
        format!(" Details loaded at {time}"),
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
                "{INDENT}{} · {} · {}",
                review.author,
                review_label(review.state),
                review.body.replace('\n', " ")
            ),
        ));
    }
    for comment in &detail.comments {
        thread.push((
            comment.created_at,
            format!(
                "{INDENT}{} · {}",
                comment.author,
                comment.body.replace('\n', " ")
            ),
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
        ChecksState::Success => "all checks passing",
        ChecksState::Failure => "at least one check failing",
        ChecksState::Pending => "checks running",
        ChecksState::None => "no checks",
    }
}

fn review_label(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Approved => "approved",
        ReviewState::ChangesRequested => "changes requested",
        ReviewState::ReviewRequired => "review required",
        ReviewState::None => "nothing to report",
    }
}

/// Pictogramme de l'état de fusion, pour aligner la ligne sur les deux
/// autres états. Un état inconnu est une attente, pas un échec : le point
/// gris dit « rien à annoncer », comme dans la liste.
fn mergeable_glyph(state: MergeableState) -> Glyph {
    match state {
        MergeableState::Mergeable => Glyph {
            symbol: '✓',
            tone: Tone::Green,
        },
        MergeableState::Conflicting => Glyph {
            symbol: '⚠',
            tone: Tone::Red,
        },
        MergeableState::Unknown => Glyph {
            symbol: '·',
            tone: Tone::Gray,
        },
    }
}

fn mergeable_label(state: MergeableState) -> &'static str {
    match state {
        MergeableState::Mergeable => "mergeable",
        MergeableState::Conflicting => "conflicts to resolve",
        // Une attente, pas un blocage : GitHub calcule ce champ à la demande.
        MergeableState::Unknown => "merge state being computed",
    }
}

/// La fenêtre de fusion, prête à dessiner : un titre de cadre et des lignes
/// déjà écrites, chevron de sélection et tons compris. `ui` ne compose rien.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeRender {
    pub title: String,
    pub lines: Vec<MergeLine>,
}

/// Une ligne de la fenêtre : des morceaux teintés, comme une ligne de liste.
/// `ui` les met bout à bout. Une ligne vide n'a aucun morceau.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeLine {
    pub cells: Vec<Cell>,
}

impl MergeLine {
    fn empty() -> Self {
        Self { cells: Vec::new() }
    }

    fn plain(text: impl Into<String>) -> Self {
        Self {
            cells: vec![Cell::plain(text)],
        }
    }

    fn toned(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            cells: vec![Cell::toned(text, tone)],
        }
    }

    /// Ligne entière, morceaux mis bout à bout. Sert au repli et aux tests.
    pub fn text(&self) -> String {
        self.cells.iter().map(|cell| cell.text.as_str()).collect()
    }
}

const MERGE_TITLE: &str = " Merge ";
const HELP_CHOOSING: &str = "Enter to confirm · Esc to cancel";
const HELP_FAILED: &str = "Enter to retry · Esc to close";
const MERGING: &str = "Merging…";

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
        MergeMethod::Squash => "Squash and merge",
        MergeMethod::Rebase => "Rebase and merge",
        MergeMethod::Merge => "Create a merge commit",
    }
}

/// Libellé d'une méthode au fil d'une phrase, sans capitale.
fn short_method_label(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Squash => "squash and merge",
        MergeMethod::Rebase => "rebase and merge",
        MergeMethod::Merge => "create a merge commit",
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

        // Les tons de l'en-tête sont ceux de la liste : dépôt en cyan,
        // séparateur et numéro en gris, titre en couleur par défaut.
        let mut lines = vec![
            MergeLine {
                cells: vec![
                    Cell::toned(dialog.key.repo.clone(), Tone::Cyan),
                    Cell::toned(SEPARATOR, Tone::Gray),
                    Cell::toned(format!("#{}", dialog.key.number), Tone::Gray),
                ],
            },
            MergeLine::plain(dialog.title.clone()),
            MergeLine::empty(),
        ];

        match &dialog.state {
            MergeDialogState::Choosing => {
                lines.push(MergeLine::plain("Method:"));
                for (index, choice) in dialog.methods.iter().enumerate() {
                    let caret = if index == dialog.selected { ">" } else { " " };
                    let text = format!("  {caret} {}", label(choice.method));
                    // Comme dans la liste, la ligne sélectionnée n'est pas
                    // surlignée : le chevron suffit. Une méthode refusée par
                    // le dépôt reste lisible, du gris des colonnes
                    // secondaires de la liste.
                    lines.push(if choice.allowed {
                        MergeLine::plain(text)
                    } else {
                        MergeLine::toned(text, Tone::Gray)
                    });
                }
                lines.push(MergeLine::empty());
                lines.push(MergeLine::plain(HELP_CHOOSING));
            }
            MergeDialogState::Submitting => {
                if let Some(method) = dialog.method() {
                    lines.push(MergeLine::plain(format!(
                        "Method: {}",
                        short_method_label(method)
                    )));
                }
                lines.push(MergeLine::empty());
                lines.push(MergeLine::plain(MERGING));
            }
            MergeDialogState::Failed(message) => {
                lines.push(MergeLine::toned(message.clone(), Tone::Red));
                lines.push(MergeLine::empty());
                lines.push(MergeLine::plain(HELP_FAILED));
            }
        }

        let lines = lines
            .iter()
            .flat_map(|line| wrap_line(line, content_width))
            .collect();

        Some(MergeRender {
            title: MERGE_TITLE.to_string(),
            lines,
        })
    }
}

/// Replie une ligne teintée. Le texte entier est replié comme un texte simple,
/// puis les tons d'origine sont reposés caractère par caractère : `wrap` ne
/// fait que retirer des espaces aux coupures, sans jamais réordonner ni
/// ajouter, donc suivre les deux suites en parallèle suffit à les retrouver.
fn wrap_line(line: &MergeLine, width: usize) -> Vec<MergeLine> {
    let toned: Vec<(char, Option<Tone>)> = line
        .cells
        .iter()
        .flat_map(|cell| cell.text.chars().map(|character| (character, cell.tone)))
        .collect();

    let mut next = 0;
    wrap(&line.text(), width)
        .into_iter()
        .map(|text| {
            let mut cells: Vec<Cell> = Vec::new();
            for character in text.chars() {
                // Les espaces mangés par la coupure sont sautés ici.
                while toned
                    .get(next)
                    .is_some_and(|(candidate, _)| *candidate != character)
                {
                    next += 1;
                }
                let tone = toned.get(next).and_then(|(_, tone)| *tone);
                next += 1;
                match cells.last_mut() {
                    Some(last) if last.tone == tone => last.text.push(character),
                    _ => cells.push(Cell {
                        text: character.to_string(),
                        tone,
                    }),
                }
            }
            MergeLine { cells }
        })
        .collect()
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

    fn rows(app: &crate::app::App, width: u16) -> Vec<ListRow> {
        match app.list_render(width) {
            ListRender::Rows(rows) => rows,
            other => panic!("rendu inattendu : {other:?}"),
        }
    }

    /// PR mise à jour il y a trois heures : l'âge affiché reste le même
    /// pendant tout le test, ce qu'une date fixe ne garantirait pas.
    fn pr_aged(number: u32) -> PrSummary {
        PrSummary {
            updated_at: Utc::now() - chrono::Duration::hours(3),
            ..pr(number)
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
    fn a_row_carries_the_glyphs_then_the_repo_the_number_the_age_the_target_and_the_title() {
        let app = app_with(vec![pr_aged(142)]);
        let row = rows(&app, LARGE).remove(0);
        assert_eq!(
            row.checks,
            Glyph {
                symbol: '✓',
                tone: Tone::Green
            }
        );
        assert_eq!(
            row.review,
            Glyph {
                symbol: '✓',
                tone: Tone::Green
            }
        );
        assert_eq!(row.text(), "moi/depot │ #142  3h  develop  Titre 142");
        assert!(!row.dim);
    }

    #[test]
    fn the_repo_and_the_number_are_told_apart_by_a_bar() {
        let app = app_with(vec![pr_aged(142)]);
        assert!(
            rows(&app, LARGE)[0].text().contains("moi/depot │ #142"),
            "le dépôt et le numéro se lisent ensemble : une espace les confondrait"
        );
    }

    #[test]
    fn each_column_carries_its_own_tone() {
        let app = app_with(vec![pr_aged(142)]);
        let cells = rows(&app, LARGE).remove(0).cells;
        let toned: Vec<(&str, Option<Tone>)> = cells
            .iter()
            .map(|cell| (cell.text.trim(), cell.tone))
            .filter(|(text, _)| !text.is_empty())
            .collect();
        assert_eq!(
            toned,
            vec![
                ("moi/depot", Some(Tone::Cyan)),
                ("│", Some(Tone::Gray)),
                ("#142", Some(Tone::Gray)),
                ("3h", Some(Tone::Gray)),
                ("develop", Some(Tone::Blue)),
                ("Titre 142", None),
            ]
        );
    }

    #[test]
    fn the_age_column_gives_minutes_then_hours_then_days() {
        let now: DateTime<Utc> = "2026-09-03T12:00:00Z".parse().expect("date valide");
        let cases = [
            ("2026-09-03T12:00:00Z", "0m"),
            ("2026-09-03T11:26:00Z", "34m"),
            ("2026-09-03T05:00:00Z", "7h"),
            ("2026-08-31T12:00:00Z", "3d"),
            // Horloges désaccordées : une date dans le futur ne donne pas de
            // nombre négatif.
            ("2026-09-03T13:00:00Z", "0m"),
        ];
        for (updated_at, expected) in cases {
            let updated_at: DateTime<Utc> = updated_at.parse().expect("date valide");
            assert_eq!(short_age(updated_at, now), expected, "date = {updated_at}");
        }
    }

    #[test]
    fn the_target_column_comes_from_the_list_without_the_detail() {
        let app = app_with(vec![PrSummary {
            base_ref: "release/2.0".to_string(),
            ..pr_aged(1)
        }]);
        assert!(
            rows(&app, LARGE)[0].text().contains("release/2.0"),
            "{}",
            rows(&app, LARGE)[0].text()
        );
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
                rows(&app, LARGE)[0].checks,
                Glyph { symbol, tone },
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
                rows(&app, LARGE)[0].review,
                Glyph { symbol, tone },
                "état = {state:?}"
            );
        }
    }

    #[test]
    fn a_draft_is_prefixed_and_dimmed() {
        let app = app_with(vec![PrSummary {
            is_draft: true,
            ..pr_aged(150)
        }]);
        let row = rows(&app, LARGE).remove(0);
        assert_eq!(
            row.text(),
            "moi/depot │ #150  3h  develop  [draft] Titre 150"
        );
        assert!(row.dim, "la ligne d'un brouillon est grisée");
    }

    #[test]
    fn a_merge_conflict_is_flagged_before_the_title() {
        let app = app_with(vec![PrSummary {
            mergeable: MergeableState::Conflicting,
            ..pr_aged(31)
        }]);
        assert_eq!(
            rows(&app, LARGE)[0].text(),
            "moi/depot │ #31  3h  develop  ⚠ Titre 31"
        );
    }

    #[test]
    fn an_unknown_merge_state_shows_nothing() {
        let app = app_with(vec![PrSummary {
            mergeable: MergeableState::Unknown,
            ..pr_aged(31)
        }]);
        assert_eq!(
            rows(&app, LARGE)[0].text(),
            "moi/depot │ #31  3h  develop  Titre 31",
            "GitHub calcule peut-être encore : ne rien annoncer"
        );
    }

    #[test]
    fn a_conflicting_draft_carries_both_marks() {
        let app = app_with(vec![PrSummary {
            is_draft: true,
            mergeable: MergeableState::Conflicting,
            ..pr_aged(7)
        }]);
        assert_eq!(
            rows(&app, LARGE)[0].text(),
            "moi/depot │ #7  3h  develop  [draft] ⚠ Titre 7"
        );
    }

    #[test]
    fn the_repos_and_the_numbers_line_up() {
        let app = app_with(vec![
            pr_in("moi/depot", 7),
            pr_in("moi/un-depot-plus-long", 150),
        ]);
        let rows = rows(&app, LARGE);
        let column = |row: &ListRow| row.text().find("│").expect("colonne du séparateur");
        assert_eq!(
            column(&rows[0]),
            column(&rows[1]),
            "les numéros commencent à la même colonne"
        );
        let title = |row: &ListRow| row.text().find("Titre").expect("colonne du titre");
        assert_eq!(title(&rows[0]), title(&rows[1]), "les titres aussi");
    }

    #[test]
    fn the_title_is_truncated_to_the_available_width() {
        let app = app_with(vec![PrSummary {
            title: "Un titre beaucoup trop long pour la fenêtre".to_string(),
            ..pr_aged(1)
        }]);
        // 40 colonnes, dont la tête de ligne que `ui` ajoute lui-même.
        let row = rows(&app, 40).remove(0);
        assert!(row.text().chars().count() <= 40, "{}", row.text());
        assert!(row.text().starts_with("moi/depot │ #1  "), "{}", row.text());
        assert!(row.text().ends_with('…'), "{}", row.text());
    }

    #[test]
    fn the_repo_and_the_number_are_never_truncated() {
        // Juste de quoi tenir la tête de ligne, le dépôt, le séparateur et
        // le numéro : les colonnes facultatives sont abandonnées entières.
        let app = app_with(vec![pr_aged(142)]);
        let row = rows(&app, (GLYPHS + 9 + 3 + 4) as u16).remove(0);
        assert_eq!(
            row.text(),
            "moi/depot │ #142",
            "pas d'âge, pas de cible, pas de titre, mais tout le reste"
        );
    }

    #[test]
    fn a_window_too_narrow_asks_to_be_widened() {
        let app = app_with(vec![pr(142)]);
        match app.list_render(10) {
            ListRender::TooNarrow(message) => {
                assert!(message.contains("Widen"), "message = {message}")
            }
            other => panic!("rendu inattendu : {other:?}"),
        }
    }

    #[test]
    fn an_empty_list_recalls_the_active_filters() {
        let app = app_with(vec![]);
        match app.list_render(LARGE) {
            ListRender::Empty(lines) => {
                assert_eq!(lines[0], "No pull requests");
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

    #[test]
    fn no_em_dash_reaches_the_interface() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));

        let mut affiche = vec![LIST_TITLE.to_string(), DETAIL_TITLE.to_string()];
        affiche.extend(rows(&app, LARGE).iter().map(ListRow::text));
        affiche.extend(merge_texts(&app));
        affiche.push(app.status_line(LARGE));
        affiche.extend(texts(&app_in_detail(1)));

        for line in affiche {
            assert!(!line.contains('—'), "tiret cadratin dans : {line}");
        }
    }

    #[test]
    fn the_header_shows_before_the_response_and_the_rest_says_loading() {
        let mut app = app_with(vec![pr(142)]);
        app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));

        let texts = texts(&app);
        let header = texts.join("\n");
        assert!(
            header.contains("moi/depot") && header.contains("#142"),
            "l'en-tête vient de PrSummary, déjà en mémoire : {texts:?}"
        );
        assert!(header.contains("Titre 142"), "{texts:?}");
        assert!(
            texts.iter().any(|line| line.contains("Loading details")),
            "{texts:?}"
        );
    }

    #[test]
    fn the_header_is_framed_and_the_sections_are_titled() {
        let texts = texts(&app_in_detail(1));
        assert!(texts[0].starts_with('┌'), "un cadre autour de l'en-tête");
        assert!(
            texts.iter().any(|line| line.starts_with('└')),
            "le cadre se ferme : {texts:?}"
        );
        for title in [
            " Branches",
            " Status",
            " Description",
            " Reviews and comments",
        ] {
            let index = texts
                .iter()
                .position(|line| line == title)
                .unwrap_or_else(|| panic!("section « {title} » attendue : {texts:?}"));
            assert!(
                texts[index - 1].is_empty(),
                "une ligne vide sépare la section précédente : {texts:?}"
            );
        }
        assert!(
            texts.iter().any(|line| line.starts_with(" Checks (")),
            "{texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|line| line.starts_with(" Files changed (")),
            "{texts:?}"
        );
    }

    #[test]
    fn the_detail_gives_the_states_in_plain_words() {
        let texts = texts(&app_in_detail(1)).join("\n");
        assert!(texts.contains("ma-branche -> develop"), "{texts}");
        // « by moi » et pas « moi » : « moi » figure déjà dans le dépôt de
        // la ligne d'en-tête, l'assertion tiendrait sans ligne d'auteur.
        assert!(texts.contains("by moi"), "l'auteur : {texts}");
        assert!(
            texts.contains("all checks passing"),
            "les mêmes états que la liste, en clair : {texts}"
        );
        assert!(texts.contains("approved"), "{texts}");
        assert!(texts.contains("mergeable"), "{texts}");
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
            texts.iter().any(|line| line.contains("Titre 1")),
            "l'en-tête vient du résumé porté par le détail en cache : {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|line| line.contains("ma-branche -> develop")),
            "le corps reste composé depuis le cache : {texts:?}"
        );
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

    /// Textes des lignes de la fenêtre ouverte.
    fn merge_texts(app: &crate::app::App) -> Vec<String> {
        app.merge_render(LARGE)
            .expect("la fenêtre doit être ouverte")
            .lines
            .iter()
            .map(MergeLine::text)
            .collect()
    }

    /// Fenêtre ouverte sur une PR dont on choisit les règles du dépôt.
    fn app_with_dialog(rules: RepoMergeRules) -> crate::app::App {
        let mut app = app_with(vec![pr_with_rules(142, rules)]);
        app.handle(Event::Key(Key::Char('m')));
        app
    }

    #[test]
    fn the_dialog_shows_the_repo_the_title_and_the_three_methods_in_order() {
        let app = app_with_dialog(all_allowed());
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");
        let texts: Vec<String> = render.lines.iter().map(MergeLine::text).collect();

        assert_eq!(render.title, " Merge ");
        assert_eq!(texts[0], "moi/depot │ #142");
        assert_eq!(texts[1], "Titre 142");
        assert_eq!(
            texts[3..7],
            [
                "Method:",
                "    Create a merge commit",
                "  > Squash and merge",
                "    Rebase and merge",
            ]
        );
        assert!(texts.contains(&"Enter to confirm · Esc to cancel".to_string()));
    }

    #[test]
    fn the_header_carries_the_tones_of_the_list() {
        let app = app_with_dialog(all_allowed());
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");

        // Le séparateur et le numéro portent le même gris : le repli les
        // rend en un seul morceau, ce qui ne change rien à l'affichage.
        assert_eq!(
            render.lines[0].cells,
            vec![
                Cell::toned("moi/depot", Tone::Cyan),
                Cell::toned(" │ #142", Tone::Gray),
            ]
        );
        // Le titre garde la couleur par défaut du terminal, comme dans la liste.
        assert_eq!(render.lines[1].cells, vec![Cell::plain("Titre 142")]);
    }

    #[test]
    fn a_method_the_repo_refuses_stays_visible_but_greyed_out() {
        let rules = RepoMergeRules {
            squash: true,
            merge: false,
            rebase: false,
            delete_branch_on_merge: true,
        };
        let app = app_with_dialog(rules);
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit être ouverte");

        let tone = |needle: &str| {
            render
                .lines
                .iter()
                .find(|line| line.text().contains(needle))
                .expect("ligne absente")
                .cells[0]
                .tone
        };
        assert_eq!(tone("Create a merge commit"), Some(Tone::Gray));
        assert_eq!(tone("Rebase and merge"), Some(Tone::Gray));
        assert_eq!(tone("Squash and merge"), None);
    }

    #[test]
    fn the_dialog_announces_the_merge_in_progress_without_closing() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        app.handle(Event::Key(Key::Enter));
        let render = app
            .merge_render(LARGE)
            .expect("la fenêtre doit rester ouverte");

        let texts: Vec<String> = render.lines.iter().map(MergeLine::text).collect();
        assert!(texts.contains(&"Merging…".to_string()));
        assert!(!texts.iter().any(|line| line.contains("Esc to cancel")));
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

        let texts: Vec<String> = render.lines.iter().map(MergeLine::text).collect();
        assert!(texts.contains(&"Base branch was modified.".to_string()));
        assert!(texts.contains(&"Enter to retry · Esc to close".to_string()));
        // Le message de GitHub est en rouge, comme les états en échec de la liste.
        assert_eq!(
            render.lines[3].cells,
            vec![Cell::toned("Base branch was modified.", Tone::Red)]
        );
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
        let render_lines: Vec<String> = render.lines.iter().map(MergeLine::text).collect();
        for line in &render_lines {
            assert!(
                line.chars().count() <= 60,
                "ligne trop longue, non repliée : {line}"
            );
        }

        // Rien n'est perdu : les morceaux du message, mis bout à bout avec
        // une espace, redonnent le message d'origine.
        let start_index = render_lines
            .iter()
            .position(|line| line.starts_with("Required"))
            .expect("le message doit apparaître");
        let end_index = render_lines
            .iter()
            .position(|line| line.ends_with("write access."))
            .expect("la fin du message doit apparaître");
        let rebuilt = render_lines[start_index..=end_index].join(" ");
        assert_eq!(rebuilt, message);
    }
}
