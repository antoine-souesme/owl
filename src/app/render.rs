//! Composition de l'affichage : pictogrammes, colonnes, troncature, messages.
//!
//! Tout ce qui se décide avant de dessiner est ici, et rien de ce qui est ici
//! ne touche au terminal. `ui` reçoit des chaînes prêtes et des tons, et
//! n'ajoute que la mise en page et la couleur.

use crate::app::App;
use crate::filter::Filter;
use crate::model::{ChecksState, MergeableState, PrSummary, ReviewState};

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
fn tronquer(texte: &str, largeur: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app::tests::{app_garnie, pr, pr_de};
    use crate::config::Config;

    /// Largeur confortable : aucun titre n'y est tronqué.
    const LARGE: u16 = 120;

    fn lignes(app: &crate::app::App, largeur: u16) -> Vec<ListRow> {
        match app.list_render(largeur) {
            ListRender::Rows(lignes) => lignes,
            autre => panic!("rendu inattendu : {autre:?}"),
        }
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
}
