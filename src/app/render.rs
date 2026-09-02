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
    Vert,
    Rouge,
    Jaune,
    Gris,
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
const PICTOGRAMMES: usize = 6;

/// Espacement entre deux colonnes de texte.
const ECART: usize = 2;

const TROP_ETROIT: &str = "Élargis le terminal : le dépôt et le numéro n'y tiennent pas.";

const LISTE_VIDE: &str = "Aucune pull request";

impl App {
    /// Compose la liste pour une largeur donnée, celle de l'intérieur du cadre.
    ///
    /// La largeur entre ici parce que la troncature en dépend, et qu'elle est
    /// une décision : `ui` ne coupe jamais un texte lui-même.
    pub fn list_render(&self, width: u16) -> ListRender {
        if self.prs.is_empty() {
            return ListRender::Empty(vec![
                LISTE_VIDE.to_string(),
                format!("Filtres actifs : {}", self.filtres_actifs()),
            ]);
        }

        let largeur = width as usize;
        let colonne_depot = self
            .prs
            .iter()
            .map(|pr| pr.key.repo.chars().count())
            .max()
            .unwrap_or(0);
        let colonne_numero = self
            .prs
            .iter()
            .map(|pr| numero(pr).chars().count())
            .max()
            .unwrap_or(0);

        let minimale = PICTOGRAMMES + colonne_depot + ECART + colonne_numero;
        if largeur < minimale {
            return ListRender::TooNarrow(TROP_ETROIT.to_string());
        }
        let titre_disponible = largeur.saturating_sub(minimale + ECART);

        ListRender::Rows(
            self.prs
                .iter()
                .map(|pr| ListRow {
                    checks: glyphe_verifications(pr.checks),
                    review: glyphe_relecture(pr.review),
                    text: ligne_texte(pr, colonne_depot, colonne_numero, titre_disponible),
                    dim: pr.is_draft,
                })
                .collect(),
        )
    }

    /// Rappel des filtres actifs, pour la liste vide.
    fn filtres_actifs(&self) -> String {
        self.filters
            .iter()
            .map(Filter::fragment)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn numero(pr: &PrSummary) -> String {
    format!("#{}", pr.key.number)
}

/// Dépôt, numéro et titre, en colonnes alignées.
fn ligne_texte(
    pr: &PrSummary,
    colonne_depot: usize,
    colonne_numero: usize,
    titre_disponible: usize,
) -> String {
    let mut ligne = format!(
        "{:<colonne_depot$}  {:<colonne_numero$}",
        pr.key.repo,
        numero(pr)
    );
    if titre_disponible > 0 {
        ligne.push_str("  ");
        ligne.push_str(&tronquer(&titre_affiche(pr), titre_disponible));
    }
    // La dernière colonne ne porte pas de remplissage inutile.
    ligne.trim_end().to_string()
}

/// Titre avec ses marques : le brouillon qualifie la pull request, le conflit
/// qualifie sa fusion. Un état de fusion inconnu n'affiche rien, GitHub étant
/// peut-être encore en train de le calculer.
fn titre_affiche(pr: &PrSummary) -> String {
    let mut titre = String::new();
    if pr.is_draft {
        titre.push_str("[brouillon] ");
    }
    if pr.mergeable == MergeableState::Conflicting {
        titre.push_str("⚠ ");
    }
    titre.push_str(&pr.title);
    titre
}

/// Coupe à la largeur donnée, en marquant la coupe. La mesure se fait en
/// caractères : compter les colonnes réellement occupées demanderait une
/// dépendance de plus.
pub(super) fn tronquer(texte: &str, largeur: usize) -> String {
    if texte.chars().count() <= largeur {
        return texte.to_string();
    }
    if largeur <= 1 {
        return texte.chars().take(largeur).collect();
    }
    let mut coupe: String = texte.chars().take(largeur - 1).collect();
    coupe.push('…');
    coupe
}

fn glyphe_verifications(etat: ChecksState) -> Glyph {
    match etat {
        ChecksState::Success => Glyph {
            symbol: '✓',
            tone: Tone::Vert,
        },
        ChecksState::Failure => Glyph {
            symbol: '✗',
            tone: Tone::Rouge,
        },
        ChecksState::Pending => Glyph {
            symbol: '○',
            tone: Tone::Jaune,
        },
        ChecksState::None => Glyph {
            symbol: '·',
            tone: Tone::Gris,
        },
    }
}

fn glyphe_relecture(etat: ReviewState) -> Glyph {
    match etat {
        ReviewState::Approved => Glyph {
            symbol: '✓',
            tone: Tone::Vert,
        },
        ReviewState::ChangesRequested => Glyph {
            symbol: '✗',
            tone: Tone::Rouge,
        },
        ReviewState::ReviewRequired => Glyph {
            symbol: '●',
            tone: Tone::Jaune,
        },
        ReviewState::None => Glyph {
            symbol: '·',
            tone: Tone::Gris,
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
    fn simple(texte: impl Into<String>) -> Self {
        Self {
            text: texte.into(),
            tone: None,
        }
    }

    fn teintee(texte: impl Into<String>, ton: Tone) -> Self {
        Self {
            text: texte.into(),
            tone: Some(ton),
        }
    }
}

const CHARGEMENT_DETAIL: &str = "Chargement du détail…";
const SANS_DESCRIPTION: &str = "(aucune description)";

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
        let Some(resume) = self.resume_affiche(key) else {
            return Vec::new();
        };

        let mut lignes = vec![
            DetailLine::simple(format!(
                "{}  #{}  {}",
                resume.key.repo, resume.key.number, resume.title
            )),
            DetailLine::simple(format!("par {}", resume.author)),
        ];

        match cache {
            None => lignes.push(DetailLine::simple(CHARGEMENT_DETAIL)),
            Some(cache) => {
                lignes.extend(corps_du_detail(
                    &cache.detail,
                    &cache.loaded_at.format("%H:%M").to_string(),
                ));
            }
        }

        // La troncature est faite en dernier, sur toutes les lignes à la fois :
        // aucune n'a le droit de dépasser la zone.
        let largeur = width as usize;
        lignes
            .into_iter()
            .map(|ligne| DetailLine {
                text: tronquer(&ligne.text, largeur),
                tone: ligne.tone,
            })
            .collect()
    }
}

/// Corps du détail, dans l'ordre de la spec : branches, états en clair,
/// description, vérifications, échanges, fichiers.
fn corps_du_detail(detail: &PrDetail, heure: &str) -> Vec<DetailLine> {
    let mut lignes = vec![
        DetailLine::simple(format!("de {} vers {}", detail.head_ref, detail.base_ref)),
        DetailLine::teintee(
            libelle_verifications(detail.summary.checks),
            glyphe_verifications(detail.summary.checks).tone,
        ),
        DetailLine::teintee(
            libelle_relecture(detail.summary.review),
            glyphe_relecture(detail.summary.review).tone,
        ),
        DetailLine::simple(libelle_fusion(detail.summary.mergeable)),
        DetailLine::simple(String::new()),
    ];

    if detail.body.trim().is_empty() {
        lignes.push(DetailLine::simple(SANS_DESCRIPTION));
    } else {
        lignes.extend(detail.body.lines().map(DetailLine::simple));
    }
    lignes.push(DetailLine::simple(String::new()));

    lignes.push(DetailLine::simple(format!(
        "Vérifications ({})",
        detail.checks.len()
    )));
    for verification in &detail.checks {
        let glyphe = glyphe_verifications(verification.state);
        lignes.push(DetailLine::teintee(
            format!("  {} {}", glyphe.symbol, verification.name),
            glyphe.tone,
        ));
    }
    lignes.push(DetailLine::simple(String::new()));

    lignes.push(DetailLine::simple("Relectures et commentaires"));
    lignes.extend(echanges(detail));
    lignes.push(DetailLine::simple(String::new()));

    lignes.push(DetailLine::simple(format!(
        "Fichiers modifiés ({}) · +{} -{}",
        detail.files.len(),
        detail.additions,
        detail.deletions
    )));
    for fichier in &detail.files {
        lignes.push(DetailLine::simple(format!(
            "  {}  +{} -{}",
            fichier.path, fichier.additions, fichier.deletions
        )));
    }

    lignes.push(DetailLine::simple(String::new()));
    lignes.push(DetailLine::teintee(
        format!("Détail chargé à {heure}"),
        Tone::Gris,
    ));
    lignes
}

/// Relectures et commentaires fondus dans un seul fil chronologique : c'est
/// l'ordre dans lequel la conversation a eu lieu.
fn echanges(detail: &PrDetail) -> Vec<DetailLine> {
    let mut fil: Vec<(chrono::DateTime<chrono::Utc>, String)> = Vec::new();
    for relecture in &detail.reviews {
        fil.push((
            relecture.submitted_at,
            format!(
                "  {} · {} · {}",
                relecture.author,
                libelle_relecture(relecture.state),
                relecture.body.replace('\n', " ")
            ),
        ));
    }
    for commentaire in &detail.comments {
        fil.push((
            commentaire.created_at,
            format!(
                "  {} · {}",
                commentaire.author,
                commentaire.body.replace('\n', " ")
            ),
        ));
    }
    fil.sort_by_key(|(instant, _)| *instant);
    fil.into_iter()
        .map(|(_, texte)| DetailLine::simple(texte))
        .collect()
}

fn libelle_verifications(etat: ChecksState) -> &'static str {
    match etat {
        ChecksState::Success => "toutes les vérifications passent",
        ChecksState::Failure => "au moins une vérification échoue",
        ChecksState::Pending => "vérifications en cours",
        ChecksState::None => "aucune vérification",
    }
}

fn libelle_relecture(etat: ReviewState) -> &'static str {
    match etat {
        ReviewState::Approved => "approuvée",
        ReviewState::ChangesRequested => "changements demandés",
        ReviewState::ReviewRequired => "relecture attendue",
        ReviewState::None => "rien à signaler",
    }
}

fn libelle_fusion(etat: MergeableState) -> &'static str {
    match etat {
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

const TITRE_FUSION: &str = " Fusionner ";
const AIDE_CHOIX: &str = "Entrée pour confirmer · Échap pour annuler";
const AIDE_ECHEC: &str = "Entrée pour réessayer · Échap pour fermer";
const FUSION_EN_COURS: &str = "Fusion en cours…";

/// Libellé d'une méthode dans la liste de choix.
fn libelle(methode: MergeMethod) -> &'static str {
    match methode {
        MergeMethod::Squash => "Écraser les commits (squash)",
        MergeMethod::Rebase => "Rebaser (rebase)",
        MergeMethod::Merge => "Créer un commit de fusion (merge)",
    }
}

/// Libellé d'une méthode au fil d'une phrase, sans capitale ni parenthèse.
fn libelle_court(methode: MergeMethod) -> &'static str {
    match methode {
        MergeMethod::Squash => "écraser les commits",
        MergeMethod::Rebase => "rebaser",
        MergeMethod::Merge => "créer un commit de fusion",
    }
}

impl App {
    /// Compose la fenêtre de fusion, s'il y en a une.
    ///
    /// La largeur n'entre pas ici : la fenêtre s'ajuste à son contenu, et le
    /// message d'erreur de GitHub n'est jamais tronqué.
    pub fn merge_render(&self) -> Option<MergeRender> {
        let fenetre = self.merge.as_ref()?;

        let mut lignes = vec![
            format!("{} #{}", fenetre.key.repo, fenetre.key.number),
            fenetre.title.clone(),
            String::new(),
        ];

        match &fenetre.state {
            MergeDialogState::Choosing => {
                // Une seule méthode autorisée : rien à choisir, on le dit.
                if let [seule] = fenetre.methods.as_slice() {
                    lignes.push(format!(
                        "Méthode : {} (imposé par le dépôt)",
                        libelle_court(*seule)
                    ));
                } else {
                    lignes.push("Méthode :".to_string());
                    for (indice, methode) in fenetre.methods.iter().enumerate() {
                        let chevron = if indice == fenetre.selected { ">" } else { " " };
                        lignes.push(format!("  {chevron} {}", libelle(*methode)));
                    }
                }
                lignes.push(String::new());
                lignes.push(AIDE_CHOIX.to_string());
            }
            MergeDialogState::Submitting => {
                if let Some(methode) = fenetre.method() {
                    lignes.push(format!("Méthode : {}", libelle_court(methode)));
                }
                lignes.push(String::new());
                lignes.push(FUSION_EN_COURS.to_string());
            }
            MergeDialogState::Failed(message) => {
                lignes.push(message.clone());
                lignes.push(String::new());
                lignes.push(AIDE_ECHEC.to_string());
            }
        }

        Some(MergeRender {
            title: TITRE_FUSION.to_string(),
            lines: lignes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app::tests::detail;
    use crate::app::tests::{app_garnie, page, pr, pr_avec_regles, pr_de};
    use crate::app::{Command, Event, Key, MergeDialogState, View};
    use crate::config::Config;
    use crate::model::RepoMergeRules;

    /// Largeur confortable : aucun titre n'y est tronqué.
    const LARGE: u16 = 120;

    fn lignes(app: &crate::app::App, largeur: u16) -> Vec<ListRow> {
        match app.list_render(largeur) {
            ListRender::Rows(lignes) => lignes,
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    /// Détail ouvert sur la PR donnée, réponse livrée.
    fn app_en_detail(numero: u32) -> crate::app::App {
        let mut app = app_garnie(vec![pr(numero)]);
        let generation = match &app.handle(Event::Key(Key::Right))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(numero).key,
            result: Ok(detail(numero)),
        });
        app
    }

    fn textes(app: &crate::app::App) -> Vec<String> {
        app.detail_lines(LARGE)
            .into_iter()
            .map(|ligne| ligne.text)
            .collect()
    }

    #[test]
    fn l_entete_est_affiche_avant_la_reponse_et_le_reste_indique_le_chargement() {
        let mut app = app_garnie(vec![pr(142)]);
        app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));

        let textes = textes(&app);
        assert!(
            textes[0].contains("moi/depot") && textes[0].contains("#142"),
            "l'en-tête vient de PrSummary, déjà en mémoire : {textes:?}"
        );
        assert!(textes[0].contains("Titre 142"), "{textes:?}");
        assert!(
            textes.iter().any(|ligne| ligne.contains("Chargement")),
            "{textes:?}"
        );
    }

    #[test]
    fn le_detail_donne_les_etats_en_clair() {
        let textes = textes(&app_en_detail(1)).join("\n");
        assert!(textes.contains("de ma-branche vers develop"), "{textes}");
        // « par moi » et pas « moi » : « moi » figure déjà dans le dépôt de
        // la ligne d'en-tête, l'assertion tiendrait sans ligne d'auteur.
        assert!(textes.contains("par moi"), "l'auteur : {textes}");
        assert!(
            textes.contains("toutes les vérifications passent"),
            "les mêmes états que la liste, en clair : {textes}"
        );
        assert!(textes.contains("approuvée"), "{textes}");
    }

    #[test]
    fn le_detail_liste_la_description_les_verifications_les_echanges_et_les_fichiers() {
        let textes = textes(&app_en_detail(1)).join("\n");
        assert!(textes.contains("Première ligne."), "{textes}");
        assert!(textes.contains("Seconde ligne."), "{textes}");
        assert!(
            textes.contains("tests"),
            "une vérification par ligne : {textes}"
        );
        assert!(textes.contains("collegue"), "une relecture : {textes}");
        assert!(textes.contains("Rebasé."), "un commentaire : {textes}");
        assert!(
            textes.contains("src/app/mod.rs") && textes.contains("+12") && textes.contains("-3"),
            "les fichiers et leurs compteurs : {textes}"
        );
    }

    #[test]
    fn les_relectures_et_les_commentaires_sont_dans_l_ordre_chronologique() {
        let textes = textes(&app_en_detail(1)).join("\n");
        let relecture = textes.find("collegue").expect("la relecture de 10:00");
        let commentaire = textes.find("Rebasé.").expect("le commentaire de 11:00");
        assert!(relecture < commentaire, "{textes}");
    }

    #[test]
    fn le_detail_porte_l_heure_de_son_chargement() {
        let app = app_en_detail(1);
        let heure = app
            .details
            .values()
            .next()
            .expect("un détail en cache")
            .loaded_at
            .format("%H:%M")
            .to_string();
        assert!(
            textes(&app).iter().any(|ligne| ligne.contains(&heure)),
            "le détail peut être périmé : autant dire quand il a été lu"
        );
    }

    #[test]
    fn une_ligne_de_detail_trop_longue_est_tronquee() {
        let app = app_en_detail(1);
        for ligne in app.detail_lines(40) {
            assert!(ligne.text.chars().count() <= 40, "ligne = {}", ligne.text);
        }
    }

    #[test]
    fn un_detail_en_cache_reste_affichable_si_la_pr_quitte_la_liste() {
        let mut app = app_en_detail(1);

        // La PR quitte la liste (fusionnée, filtrée...) pendant que le détail reste ouvert.
        let generation_liste = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: generation_liste,
            result: Ok(page(vec![])),
        });

        let textes = textes(&app);
        assert!(
            textes[0].contains("Titre 1"),
            "l'en-tête vient du résumé porté par le détail en cache : {textes:?}"
        );
        assert!(
            textes
                .iter()
                .any(|ligne| ligne.contains("de ma-branche vers develop")),
            "le corps reste composé depuis le cache : {textes:?}"
        );
    }

    #[test]
    fn une_ligne_porte_les_deux_pictogrammes_puis_le_depot_le_numero_et_le_titre() {
        let app = app_garnie(vec![pr(142)]);
        let ligne = lignes(&app, LARGE).remove(0);
        assert_eq!(
            ligne.checks,
            Glyph {
                symbol: '✓',
                tone: Tone::Vert
            }
        );
        assert_eq!(
            ligne.review,
            Glyph {
                symbol: '✓',
                tone: Tone::Vert
            }
        );
        assert_eq!(ligne.text, "moi/depot  #142  Titre 142");
        assert!(!ligne.dim);
    }

    #[test]
    fn chaque_etat_de_verification_a_son_pictogramme() {
        let cas = [
            (ChecksState::Success, '✓', Tone::Vert),
            (ChecksState::Failure, '✗', Tone::Rouge),
            (ChecksState::Pending, '○', Tone::Jaune),
            (ChecksState::None, '·', Tone::Gris),
        ];
        for (etat, symbole, ton) in cas {
            let app = app_garnie(vec![PrSummary {
                checks: etat,
                ..pr(1)
            }]);
            assert_eq!(
                lignes(&app, LARGE)[0].checks,
                Glyph {
                    symbol: symbole,
                    tone: ton
                },
                "état = {etat:?}"
            );
        }
    }

    #[test]
    fn chaque_etat_de_relecture_a_son_pictogramme() {
        let cas = [
            (ReviewState::Approved, '✓', Tone::Vert),
            (ReviewState::ChangesRequested, '✗', Tone::Rouge),
            (ReviewState::ReviewRequired, '●', Tone::Jaune),
            (ReviewState::None, '·', Tone::Gris),
        ];
        for (etat, symbole, ton) in cas {
            let app = app_garnie(vec![PrSummary {
                review: etat,
                ..pr(1)
            }]);
            assert_eq!(
                lignes(&app, LARGE)[0].review,
                Glyph {
                    symbol: symbole,
                    tone: ton
                },
                "état = {etat:?}"
            );
        }
    }

    #[test]
    fn un_brouillon_est_prefixe_et_grise() {
        let app = app_garnie(vec![PrSummary {
            is_draft: true,
            ..pr(150)
        }]);
        let ligne = lignes(&app, LARGE).remove(0);
        assert_eq!(ligne.text, "moi/depot  #150  [brouillon] Titre 150");
        assert!(ligne.dim, "la ligne d'un brouillon est grisée");
    }

    #[test]
    fn un_conflit_de_fusion_est_signale_devant_le_titre() {
        let app = app_garnie(vec![PrSummary {
            mergeable: MergeableState::Conflicting,
            ..pr(31)
        }]);
        assert_eq!(lignes(&app, LARGE)[0].text, "moi/depot  #31  ⚠ Titre 31");
    }

    #[test]
    fn un_etat_de_fusion_inconnu_n_affiche_rien() {
        let app = app_garnie(vec![PrSummary {
            mergeable: MergeableState::Unknown,
            ..pr(31)
        }]);
        assert_eq!(
            lignes(&app, LARGE)[0].text,
            "moi/depot  #31  Titre 31",
            "GitHub calcule peut-être encore : ne rien annoncer"
        );
    }

    #[test]
    fn un_brouillon_en_conflit_porte_les_deux_marques() {
        let app = app_garnie(vec![PrSummary {
            is_draft: true,
            mergeable: MergeableState::Conflicting,
            ..pr(7)
        }]);
        assert_eq!(
            lignes(&app, LARGE)[0].text,
            "moi/depot  #7  [brouillon] ⚠ Titre 7"
        );
    }

    #[test]
    fn les_depots_et_les_numeros_sont_alignes_entre_eux() {
        let app = app_garnie(vec![
            pr_de("moi/depot", 7),
            pr_de("moi/un-depot-plus-long", 150),
        ]);
        let lignes = lignes(&app, LARGE);
        let colonne = |ligne: &ListRow| ligne.text.find("  #").expect("colonne du numéro");
        assert_eq!(
            colonne(&lignes[0]),
            colonne(&lignes[1]),
            "les numéros commencent à la même colonne"
        );
        let titre = |ligne: &ListRow| ligne.text.find("Titre").expect("colonne du titre");
        assert_eq!(titre(&lignes[0]), titre(&lignes[1]), "les titres aussi");
    }

    #[test]
    fn le_titre_est_tronque_a_la_largeur_disponible() {
        let app = app_garnie(vec![PrSummary {
            title: "Un titre beaucoup trop long pour la fenêtre".to_string(),
            ..pr(1)
        }]);
        // 30 colonnes moins les 6 des pictogrammes, que `ui` ajoute lui-même.
        let ligne = lignes(&app, 30).remove(0);
        assert_eq!(ligne.text.chars().count(), 24);
        assert!(ligne.text.starts_with("moi/depot  #1  "), "{}", ligne.text);
        assert!(ligne.text.ends_with('…'), "{}", ligne.text);
    }

    #[test]
    fn le_depot_et_le_numero_ne_sont_jamais_tronques() {
        // Juste de quoi tenir les pictogrammes, le dépôt et le numéro.
        let app = app_garnie(vec![pr(142)]);
        let ligne = lignes(&app, 6 + 9 + 2 + 4).remove(0);
        assert_eq!(
            ligne.text, "moi/depot  #142",
            "pas de titre, mais tout le reste"
        );
    }

    #[test]
    fn une_fenetre_trop_etroite_demande_l_elargissement() {
        let app = app_garnie(vec![pr(142)]);
        match app.list_render(10) {
            ListRender::TooNarrow(message) => {
                assert!(message.contains("Élargis"), "message = {message}")
            }
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    #[test]
    fn une_liste_vide_rappelle_les_filtres_actifs() {
        let app = app_garnie(vec![]);
        match app.list_render(LARGE) {
            ListRender::Empty(lignes) => {
                assert_eq!(lignes[0], "Aucune pull request");
                assert!(
                    lignes[1].contains("author:@me") && lignes[1].contains("is:open"),
                    "un filtre trop restrictif ressemble sinon à une panne : {}",
                    lignes[1]
                );
            }
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    #[test]
    fn une_liste_vide_avec_des_filtres_inhabituels_les_rappelle_aussi() {
        let reglages = Config {
            filters: vec!["org:acme".to_string(), "involves:@me".to_string()],
            ..Config::default()
        };
        let app = crate::app::App::new(reglages);
        match app.list_render(LARGE) {
            ListRender::Empty(lignes) => {
                assert!(lignes[1].contains("org:acme"), "{}", lignes[1]);
                assert!(lignes[1].contains("involves:@me"), "{}", lignes[1]);
            }
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    fn tout_autorise() -> RepoMergeRules {
        RepoMergeRules {
            squash: true,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        }
    }

    #[test]
    fn sans_fenetre_ouverte_il_n_y_a_rien_a_dessiner() {
        let app = app_garnie(vec![pr_avec_regles(1, tout_autorise())]);
        assert!(app.merge_render().is_none());
    }

    #[test]
    fn la_fenetre_montre_le_depot_le_titre_et_les_methodes_autorisees() {
        let mut app = app_garnie(vec![pr_avec_regles(142, tout_autorise())]);
        app.handle(Event::Key(Key::Char('m')));
        let rendu = app.merge_render().expect("la fenêtre doit être ouverte");

        assert_eq!(rendu.title, " Fusionner ");
        assert_eq!(rendu.lines[0], "moi/depot #142");
        assert_eq!(rendu.lines[1], "Titre 142");
        assert!(rendu.lines.contains(&"Méthode :".to_string()));
        assert!(rendu
            .lines
            .contains(&"  > Écraser les commits (squash)".to_string()));
        assert!(rendu.lines.contains(&"    Rebaser (rebase)".to_string()));
        assert!(rendu
            .lines
            .contains(&"    Créer un commit de fusion (merge)".to_string()));
        assert!(rendu
            .lines
            .contains(&"Entrée pour confirmer · Échap pour annuler".to_string()));
    }

    #[test]
    fn une_seule_methode_autorisee_remplace_la_liste_par_une_ligne() {
        let regles = RepoMergeRules {
            squash: true,
            merge: false,
            rebase: false,
            delete_branch_on_merge: true,
        };
        let mut app = app_garnie(vec![pr_avec_regles(142, regles)]);
        app.handle(Event::Key(Key::Char('m')));
        let rendu = app.merge_render().expect("la fenêtre doit être ouverte");

        assert!(rendu
            .lines
            .contains(&"Méthode : écraser les commits (imposé par le dépôt)".to_string()));
        assert!(!rendu.lines.iter().any(|ligne| ligne.contains('>')));
    }

    #[test]
    fn la_fenetre_annonce_la_fusion_en_cours_sans_se_fermer() {
        let mut app = app_garnie(vec![pr_avec_regles(142, tout_autorise())]);
        app.handle(Event::Key(Key::Char('m')));
        app.handle(Event::Key(Key::Enter));
        let rendu = app.merge_render().expect("la fenêtre doit rester ouverte");

        assert!(rendu.lines.contains(&"Fusion en cours…".to_string()));
        assert!(!rendu
            .lines
            .iter()
            .any(|ligne| ligne.contains("Échap pour annuler")));
    }

    #[test]
    fn un_echec_montre_le_message_de_github_tel_quel() {
        let mut app = app_garnie(vec![pr_avec_regles(142, tout_autorise())]);
        app.handle(Event::Key(Key::Char('m')));
        if let Some(fenetre) = app.merge.as_mut() {
            fenetre.state = MergeDialogState::Failed("Base branch was modified.".to_string());
        }
        let rendu = app.merge_render().expect("la fenêtre doit être ouverte");

        assert!(rendu
            .lines
            .contains(&"Base branch was modified.".to_string()));
        assert!(rendu
            .lines
            .contains(&"Entrée pour réessayer · Échap pour fermer".to_string()));
    }
}
