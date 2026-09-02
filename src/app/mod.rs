//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

mod render;

pub use render::{ListRender, ListRow, Tone};

use render::tronquer;

use std::collections::HashMap;

use chrono::{DateTime, Local};

use crate::config::Config;
use crate::filter::{self, Filter};
use crate::github::GithubError;
use crate::model::{ListPage, PrDetail, PrKey, PrSummary, RateLimit};

/// Numéro de génération d'une demande réseau. Un résultat dont la génération
/// est périmée est ignoré, ce qui évite qu'une réponse lente écrase une
/// réponse plus récente.
pub type Generation = u64;

/// Touche reçue, traduite par `main`. `app` ignore volontairement `crossterm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Char(char),
    /// Interruption clavier. Le mode brut l'a désarmée : c'est à `owl` de la
    /// traiter, sans quoi l'utilisateur n'aurait plus le réflexe habituel.
    CtrlC,
    /// Toute autre touche, sans effet.
    Other,
}

/// Ce qui arrive dans la file d'événements.
// `DetailLoaded` porte un `PrDetail` complet, nettement plus gros que les
// autres variantes : la spec le nomme ainsi, et boîter cette variante pour
// satisfaire clippy compliquerait chaque appelant sans bénéfice réel.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    Key(Key),
    /// Tour de minuteur de rafraîchissement.
    Tick,
    /// Arrêt demandé par `main` : panique d'une tâche, ou clavier hors service.
    Quit,
    /// Résultat d'une requête de liste.
    ListLoaded {
        generation: Generation,
        result: Result<ListPage, GithubError>,
    },
    /// Résultat d'une requête de détail.
    DetailLoaded {
        generation: Generation,
        key: PrKey,
        result: Result<PrDetail, GithubError>,
    },
}

/// Ce que `app` demande à `main` de faire.
#[derive(Debug, PartialEq)]
pub enum Command {
    FetchList {
        generation: Generation,
        /// Chaîne de recherche complète, assemblée par `filter::build_query`.
        query: String,
        page_size: u16,
    },
    FetchDetail {
        generation: Generation,
        /// Le résumé entier, et pas la seule clé : `github::fetch_detail` le
        /// recopie dans le `PrDetail` qu'il rend.
        summary: PrSummary,
    },
    /// Ouvre une URL dans le navigateur. `app` choisit l'URL, `main` l'ouvre :
    /// `app` ne fait aucun effet de bord lui-même.
    OpenInBrowser {
        url: String,
    },
    Quit,
}

/// Requêtes en vol. Deux drapeaux distincts : une liste qui se rafraîchit ne
/// doit pas faire croire que le détail affiché est en train de charger.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Loading {
    pub list: bool,
    pub detail: bool,
}

/// Aide clavier, en fin de barre d'état. Le texte est ici, pas dans `ui`.
const AIDE_LISTE: &str =
    "↑↓ naviguer · → détail · m fusionner · r rafraîchir · o navigateur · q quitter";
const AIDE_DETAIL: &str =
    "↑↓ défiler · ← liste · m fusionner · r rafraîchir · o navigateur · q quitter";

/// Message affiché tant qu'aucune réponse n'est arrivée.
const ATTENTE_INITIALE: &str = "Chargement…";

/// Vue affichée. Le défilement voyage avec la vue : revenir à la liste puis
/// rouvrir un détail le remet en haut, ce qui est le comportement attendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    List,
    Detail { key: PrKey, scroll: u16 },
}

/// Un détail en cache, avec l'heure de son chargement : le détail peut être
/// périmé sans qu'on le sache, autant dire quand il a été lu.
#[derive(Debug, Clone)]
pub struct CachedDetail {
    pub detail: PrDetail,
    pub loaded_at: DateTime<Local>,
}

pub struct App {
    pub prs: Vec<PrSummary>,
    /// Indice de la ligne sélectionnée. Vaut 0 sur une liste vide, où
    /// `selected_pr` ne rend alors rien.
    pub selected: usize,
    /// Clé de la ligne sélectionnée, retenue pour retrouver la sélection
    /// après un rafraîchissement qui a réordonné la liste.
    selected_key: Option<PrKey>,
    pub loading: Loading,
    /// Dernière erreur reçue, reprise telle quelle de GitHub. Effacée par la
    /// première réponse réussie.
    pub error: Option<String>,
    /// Solde d'appels rapporté par la dernière requête réussie. La suspension
    /// du rafraîchissement qu'il déclenche appartient à `05-erreurs-et-tests.md`.
    #[allow(dead_code)]
    pub rate_limit: Option<RateLimit>,
    pub should_quit: bool,
    pub last_refresh: Option<DateTime<Local>>,
    pub view: View,
    pub details: HashMap<PrKey, CachedDetail>,
    list_generation: Generation,
    detail_generation: Generation,
    /// Filtres des réglages, traduits une seule fois.
    filters: Vec<Filter>,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            prs: Vec::new(),
            selected: 0,
            selected_key: None,
            loading: Loading::default(),
            error: None,
            rate_limit: None,
            should_quit: false,
            last_refresh: None,
            view: View::List,
            details: HashMap::new(),
            list_generation: 0,
            detail_generation: 0,
            filters: config
                .filters
                .iter()
                .map(|texte| Filter::parse(texte))
                .collect(),
            config,
        }
    }

    /// Première demande, au démarrage.
    pub fn start(&mut self) -> Vec<Command> {
        vec![self.fetch_list()]
    }

    /// Pull request sélectionnée, s'il y en a une.
    pub fn selected_pr(&self) -> Option<&PrSummary> {
        self.prs.get(self.selected)
    }

    pub fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Key(touche) => self.handle_key(touche),
            Event::Quit => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            // Une requête de liste déjà en vol suffit : relancer ne ferait
            // qu'ajouter un appel et jeter la réponse précédente.
            Event::Tick => {
                if self.loading.list {
                    Vec::new()
                } else {
                    vec![self.fetch_list()]
                }
            }
            Event::ListLoaded { generation, result } => {
                // Réponse d'une demande dépassée : on la jette sans rien changer.
                if generation != self.list_generation {
                    return Vec::new();
                }
                self.loading.list = false;
                match result {
                    Ok(page) => {
                        self.apply_list(page.pull_requests);
                        self.rate_limit = page.rate_limit;
                        self.last_refresh = Some(Local::now());
                        self.error = None;
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(erreur) => self.error = Some(erreur.to_string()),
                }
                Vec::new()
            }
            Event::DetailLoaded {
                generation,
                key,
                result,
            } => {
                if generation != self.detail_generation {
                    return Vec::new();
                }
                self.loading.detail = false;
                match result {
                    Ok(detail) => {
                        self.details.insert(
                            key,
                            CachedDetail {
                                detail,
                                loaded_at: Local::now(),
                            },
                        );
                        self.error = None;
                    }
                    Err(erreur) => self.error = Some(erreur.to_string()),
                }
                self.borner_le_defilement();
                Vec::new()
            }
        }
    }

    fn handle_key(&mut self, touche: Key) -> Vec<Command> {
        // Touches communes aux deux vues, traitées avant l'aiguillage.
        match touche {
            Key::Char('q') | Key::CtrlC => {
                self.should_quit = true;
                return vec![Command::Quit];
            }
            Key::Char('r') => return self.refresh(),
            Key::Char('o') => return self.open_in_browser(),
            // La fenêtre de fusion et ses contrôles appartiennent à
            // `04-fusion.md`. La touche est reconnue ici pour ne pas tomber
            // dans un bras de navigation, et ne fait rien de plus.
            Key::Char('m') => return Vec::new(),
            _ => {}
        }

        match self.view {
            View::List => self.handle_key_list(touche),
            View::Detail { .. } => self.handle_key_detail(touche),
        }
    }

    fn handle_key_list(&mut self, touche: Key) -> Vec<Command> {
        match touche {
            Key::Up | Key::Char('k') => self.select_previous(),
            Key::Down | Key::Char('j') => self.select_next(),
            Key::Right | Key::Enter => return self.open_detail(),
            _ => {}
        }
        Vec::new()
    }

    fn handle_key_detail(&mut self, touche: Key) -> Vec<Command> {
        match touche {
            Key::Up | Key::Char('k') => self.scroll_detail(-1),
            Key::Down | Key::Char('j') => self.scroll_detail(1),
            Key::Left | Key::Esc => self.view = View::List,
            _ => {}
        }
        Vec::new()
    }

    /// `r` rafraîchit ce qui est affiché : la liste, ou le détail ouvert.
    /// Sur le détail, la requête part même si le cache répond déjà : c'est
    /// justement le seul moyen de le rafraîchir.
    fn refresh(&mut self) -> Vec<Command> {
        match &self.view {
            View::List => vec![self.fetch_list()],
            View::Detail { key, .. } => {
                let cle = key.clone();
                match self.resume_affiche(&cle).cloned() {
                    Some(resume) => vec![self.fetch_detail(resume)],
                    // Ni dans la liste, ni en cache : rien à recharger.
                    None => Vec::new(),
                }
            }
        }
    }

    /// Ouvre le détail de la sélection. Une PR déjà consultée pendant la
    /// session s'affiche depuis le cache, sans nouvelle requête.
    fn open_detail(&mut self) -> Vec<Command> {
        let Some(resume) = self.selected_pr().cloned() else {
            return Vec::new();
        };
        self.view = View::Detail {
            key: resume.key.clone(),
            scroll: 0,
        };
        if self.details.contains_key(&resume.key) {
            return Vec::new();
        }
        vec![self.fetch_detail(resume)]
    }

    /// Résumé de la pull request d'une clé donnée : celui de la liste, ou à
    /// défaut celui que porte le détail en cache.
    ///
    /// Le repli n'est pas un détail d'affichage : quand un rafraîchissement
    /// retire la PR alors que son détail reste ouvert, c'est la seule source
    /// du résumé, et `o` comme `r` en ont besoin autant que le dessin.
    pub(crate) fn resume_affiche(&self, key: &PrKey) -> Option<&PrSummary> {
        self.prs
            .iter()
            .find(|pr| &pr.key == key)
            .or_else(|| self.details.get(key).map(|cache| &cache.detail.summary))
    }

    /// URL de la pull request affichée : la sélection dans la liste, la PR
    /// ouverte dans le détail.
    fn open_in_browser(&self) -> Vec<Command> {
        let resume = match &self.view {
            View::List => self.selected_pr(),
            View::Detail { key, .. } => self.resume_affiche(key),
        };
        match resume {
            Some(pr) => vec![Command::OpenInBrowser {
                url: pr.url.clone(),
            }],
            None => Vec::new(),
        }
    }

    fn fetch_detail(&mut self, summary: PrSummary) -> Command {
        self.detail_generation += 1;
        self.loading.detail = true;
        Command::FetchDetail {
            generation: self.detail_generation,
            summary,
        }
    }

    /// Défilement courant de la vue détail. Zéro dans la vue liste.
    pub fn detail_scroll(&self) -> u16 {
        match &self.view {
            View::Detail { scroll, .. } => *scroll,
            View::List => 0,
        }
    }

    /// Défile de `pas` lignes, borné au contenu. La hauteur de la zone n'est
    /// pas connue ici : la dernière ligne reste atteignable, et le dessin ne
    /// peut de toute façon pas défiler au-delà.
    fn scroll_detail(&mut self, pas: i32) {
        let maximum = i64::from(self.derniere_ligne_du_detail());
        if let View::Detail { scroll, .. } = &mut self.view {
            let vise = i64::from(*scroll) + i64::from(pas);
            *scroll = vise.clamp(0, maximum) as u16;
        }
    }

    /// Indice de la dernière ligne du détail, saturé plutôt que tronqué : un
    /// détail de plus de 65 536 lignes ne doit pas ramener le défilement en
    /// haut de page.
    fn derniere_ligne_du_detail(&self) -> u16 {
        u16::try_from(self.detail_line_count().saturating_sub(1)).unwrap_or(u16::MAX)
    }

    /// Re-borne le défilement au contenu. Un détail rechargé peut être plus
    /// court que le précédent : sans cela, l'écran resterait vide jusqu'à ce
    /// que l'utilisateur remonte.
    fn borner_le_defilement(&mut self) {
        let maximum = self.derniere_ligne_du_detail();
        if let View::Detail { scroll, .. } = &mut self.view {
            *scroll = (*scroll).min(maximum);
        }
    }

    /// Range une nouvelle liste en préservant la sélection.
    ///
    /// La clé retenue est cherchée dans la nouvelle liste. Si la PR a disparu,
    /// l'indice précédent est conservé, borné à la nouvelle taille : c'est ce
    /// qui garde le curseur au même endroit de l'écran plutôt que de le
    /// renvoyer en tête à chaque fusion.
    fn apply_list(&mut self, prs: Vec<PrSummary>) {
        let precedent = self.selected;
        self.prs = prs;
        self.selected = match &self.selected_key {
            Some(cle) => self
                .prs
                .iter()
                .position(|pr| &pr.key == cle)
                .unwrap_or(precedent),
            None => precedent,
        };
        self.selected = self.selected.min(self.prs.len().saturating_sub(1));
        self.remember_selection();
    }

    fn select_previous(&mut self) {
        // La sélection ne boucle pas : en haut de liste, rien ne se passe.
        if self.selected > 0 {
            self.selected -= 1;
            self.remember_selection();
        }
    }

    fn select_next(&mut self) {
        if self.selected + 1 < self.prs.len() {
            self.selected += 1;
            self.remember_selection();
        }
    }

    fn remember_selection(&mut self) {
        self.selected_key = self.selected_pr().map(|pr| pr.key.clone());
    }

    /// Ouvre une nouvelle génération et demande la liste.
    fn fetch_list(&mut self) -> Command {
        self.list_generation += 1;
        self.loading.list = true;
        Command::FetchList {
            generation: self.list_generation,
            query: filter::build_query(&self.filters),
            page_size: self.config.page_size,
        }
    }

    /// Barre d'état complète, prête à dessiner telle quelle, pour la largeur
    /// donnée.
    ///
    /// Assemblée ici, et pas dans `ui`, parce que chaque morceau est une
    /// décision : le libellé de l'heure, l'annonce d'une requête en cours,
    /// l'aide clavier. La largeur en est une aussi : à 80 colonnes, l'aide
    /// seule ne tient pas avec le reste, et il faut choisir plutôt que laisser
    /// la bibliothèque de dessin rogner la ligne en silence.
    ///
    /// Règle : chaque morceau porte son rang de sacrifice ; tant que la ligne
    /// dépasse, le rang le plus élevé est retiré. L'aide part la première —
    /// c'est un rappel, pas une information — puis l'heure, puis le résumé de
    /// la liste, puis l'annonce du chargement. L'erreur en cours reste, et
    /// n'est tronquée qu'en dernier recours, si la largeur ne tient même pas
    /// un morceau.
    pub fn status_line(&self, width: u16) -> String {
        // Rangs de sacrifice, du plus retenu au premier lâché.
        const ERREUR: u8 = 0;
        const CHARGEMENT: u8 = 1;
        const RESUME: u8 = 2;
        const HEURE: u8 = 3;
        const AIDE: u8 = 4;

        let mut morceaux: Vec<(u8, String)> = Vec::new();

        if self.last_refresh.is_none() && self.error.is_none() {
            // Rien n'est encore arrivé : l'attente est le message principal.
            morceaux.push((ERREUR, ATTENTE_INITIALE.to_string()));
        } else {
            if self.last_refresh.is_some() {
                morceaux.push((RESUME, self.liste_resumee()));
            }
            if let Some(instant) = self.last_refresh {
                morceaux.push((HEURE, format!("mis à jour à {}", instant.format("%H:%M"))));
            }
            if self.loading.list || self.loading.detail {
                morceaux.push((CHARGEMENT, "chargement…".to_string()));
            }
            if let Some(erreur) = &self.error {
                morceaux.push((ERREUR, erreur.clone()));
            }
        }

        morceaux.push((
            AIDE,
            match self.view {
                View::List => AIDE_LISTE,
                View::Detail { .. } => AIDE_DETAIL,
            }
            .to_string(),
        ));

        let largeur = width as usize;
        while morceaux.len() > 1 && assembler(&morceaux).chars().count() > largeur {
            let sacrifie = morceaux
                .iter()
                .enumerate()
                .max_by_key(|(_, (rang, _))| *rang)
                .map(|(indice, _)| indice)
                .expect("la liste n'est pas vide");
            morceaux.remove(sacrifie);
        }
        tronquer(&assembler(&morceaux), largeur)
    }

    /// Résumé de la liste pour la barre d'état, jamais tronqué autrement que
    /// par le retrait d'un morceau entier.
    fn liste_resumee(&self) -> String {
        match self.prs.len() {
            0 => "Aucune pull request".to_string(),
            1 => "1 pull request".to_string(),
            nombre => format!("{nombre} pull requests"),
        }
    }
}

/// Assemble les morceaux retenus de la barre d'état, dans l'ordre d'affichage.
fn assembler(morceaux: &[(u8, String)]) -> String {
    morceaux
        .iter()
        .map(|(_, texte)| texte.as_str())
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use crate::model::{
        ChangedFile, CheckRun, ChecksState, Comment, MergeableState, RepoMergeRules, Review,
        ReviewState,
    };

    /// Largeur où la barre d'état tient tout entière, aide comprise.
    const CONFORTABLE: u16 = 200;

    pub(crate) fn pr(numero: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: "moi/depot".to_string(),
                number: numero,
            },
            title: format!("Titre {numero}"),
            author: "moi".to_string(),
            url: format!("https://github.com/moi/depot/pull/{numero}"),
            is_draft: false,
            checks: ChecksState::Success,
            review: ReviewState::Approved,
            mergeable: MergeableState::Mergeable,
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            },
        }
    }

    /// Réponse de liste sans solde d'appels, suffisante partout où seul le
    /// contenu de la liste compte.
    pub(crate) fn page(pull_requests: Vec<PrSummary>) -> ListPage {
        ListPage {
            pull_requests,
            rate_limit: None,
        }
    }

    /// Application démarrée, première requête émise, génération courante rendue.
    pub(crate) fn app_demarree() -> (App, Generation) {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        let generation = match &commandes[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        (app, generation)
    }

    /// PR d'un dépôt donné, pour distinguer deux clés dans un même test.
    pub(crate) fn pr_de(depot: &str, numero: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: depot.to_string(),
                number: numero,
            },
            ..pr(numero)
        }
    }

    /// Application démarrée et garnie de la liste donnée.
    pub(crate) fn app_garnie(liste: Vec<PrSummary>) -> App {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(liste)),
        });
        app
    }

    /// Rafraîchit et livre la nouvelle liste, en respectant la génération.
    pub(crate) fn rafraichir(app: &mut App, liste: Vec<PrSummary>) {
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(liste)),
        });
    }

    #[test]
    fn le_demarrage_emet_une_seule_requete() {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        assert_eq!(
            commandes,
            vec![Command::FetchList {
                generation: 1,
                query: "is:pr author:@me is:open sort:updated-desc".to_string(),
                page_size: 50,
            }]
        );
        assert!(app.loading.list);
    }

    #[test]
    fn q_demande_la_sortie() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn r_relance_une_requete_avec_une_generation_plus_recente() {
        let (mut app, premiere) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Char('r')));
        match &commandes[0] {
            Command::FetchList { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert!(app.loading.list);
    }

    #[test]
    fn le_minuteur_relance_une_requete() {
        let (mut app, premiere) = app_demarree();
        app.handle(Event::ListLoaded {
            generation: premiere,
            result: Ok(page(vec![])),
        });
        let commandes = app.handle(Event::Tick);
        match &commandes[0] {
            Command::FetchList { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn une_touche_inconnue_ne_fait_rien() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Other));
        assert!(commandes.is_empty());
        assert!(!app.should_quit);
    }

    #[test]
    fn un_resultat_a_jour_remplace_la_liste() {
        let (mut app, generation) = app_demarree();
        let commandes = app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(commandes.is_empty());
        assert_eq!(app.prs, vec![pr(1), pr(2)]);
        assert!(!app.loading.list);
        assert!(app.last_refresh.is_some());
    }

    #[test]
    fn un_resultat_perime_est_ignore() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        // Une nouvelle requête part, puis la réponse lente de l'ancienne arrive.
        app.handle(Event::Key(Key::Char('r')));
        let commandes = app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(99)])),
        });
        assert!(commandes.is_empty());
        assert_eq!(
            app.prs,
            vec![pr(1)],
            "la réponse lente ne doit rien écraser"
        );
        assert!(app.loading.list, "la requête en cours reste en cours");
    }

    #[test]
    fn une_erreur_laisse_la_liste_affichee() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.prs, vec![pr(1)], "la liste précédente reste visible");
        assert_eq!(app.error.as_deref(), Some("Réseau injoignable."));
        assert!(!app.loading.list);
    }

    #[test]
    fn un_succes_efface_l_erreur_en_cours() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(app.error.is_none(), "error = {:?}", app.error);
    }

    #[test]
    fn le_status_annonce_le_nombre_de_pull_requests() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(
            app.status_line(CONFORTABLE).starts_with("2 pull requests"),
            "{}",
            app.status_line(CONFORTABLE)
        );

        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![])),
        });
        assert!(
            app.status_line(CONFORTABLE)
                .starts_with("Aucune pull request"),
            "{}",
            app.status_line(CONFORTABLE)
        );
    }

    #[test]
    fn q_pendant_un_chargement_quitte_quand_meme() {
        let (mut app, _) = app_demarree();
        assert!(app.loading.list, "une requête est bien en cours");
        let commandes = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn un_evenement_quit_arrete_la_boucle() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Quit);
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn la_barre_d_etat_au_demarrage_n_annonce_l_attente_qu_une_fois() {
        let (app, _) = app_demarree();
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("Chargement… · {AIDE_LISTE}")
        );
    }

    #[test]
    fn la_barre_d_etat_apres_une_reponse_donne_l_heure_et_l_aide() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("2 pull requests · mis à jour à {heure} · {AIDE_LISTE}")
        );
    }

    #[test]
    fn la_barre_d_etat_annonce_le_chargement_d_un_rafraichissement() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        app.handle(Event::Key(Key::Char('r')));
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("1 pull request · mis à jour à {heure} · chargement… · {AIDE_LISTE}")
        );
    }

    #[test]
    fn la_barre_d_etat_reprend_l_erreur_telle_quelle() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("Réseau injoignable. · {AIDE_LISTE}"),
            "aucune heure : aucun rafraîchissement n'a encore réussi"
        );
    }

    #[test]
    fn la_barre_d_etat_ne_depasse_jamais_la_largeur_donnee() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        // 80 colonnes : la largeur d'un terminal standard, où l'aide seule ne
        // tient déjà pas avec le résumé et l'heure.
        for largeur in [1, 12, 40, 80, 117, CONFORTABLE] {
            let barre = app.status_line(largeur);
            assert!(
                barre.chars().count() <= largeur as usize,
                "largeur {largeur} : {barre}"
            );
        }
        app.handle(Event::Key(Key::Right));
        for largeur in [1, 40, 80, CONFORTABLE] {
            let barre = app.status_line(largeur);
            assert!(
                barre.chars().count() <= largeur as usize,
                "en vue détail, largeur {largeur} : {barre}"
            );
        }
    }

    #[test]
    fn une_barre_d_etat_etroite_sacrifie_l_aide_avant_le_reste() {
        let app = app_garnie(vec![pr(1), pr(2)]);
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();

        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("2 pull requests · mis à jour à {heure} · {AIDE_LISTE}"),
            "au large, tout tient"
        );

        let etroite = app.status_line(80);
        assert!(
            !etroite.contains("naviguer"),
            "l'aide est un rappel : elle part la première ({etroite})"
        );
        assert_eq!(
            etroite,
            format!("2 pull requests · mis à jour à {heure}"),
            "le résumé et l'heure restent entiers"
        );
    }

    #[test]
    fn une_barre_d_etat_tres_etroite_garde_l_erreur() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(30),
            "Réseau injoignable.",
            "l'erreur est ce qu'on garde en dernier"
        );
    }

    #[test]
    fn l_aide_clavier_suit_la_vue_affichee() {
        let mut app = app_garnie(vec![pr(1)]);
        assert!(
            app.status_line(CONFORTABLE).ends_with(AIDE_LISTE),
            "{}",
            app.status_line(CONFORTABLE)
        );

        app.handle(Event::Key(Key::Right));
        let barre = app.status_line(CONFORTABLE);
        assert!(barre.ends_with(AIDE_DETAIL), "{barre}");
        assert!(
            barre.contains("← liste") && !barre.contains("→ détail"),
            "une touche sans effet dans la vue n'est pas rappelée : {barre}"
        );
    }

    #[test]
    fn les_filtres_des_reglages_sont_transmis_a_la_requete() {
        let reglages = Config {
            filters: vec!["review-requested:@me".to_string()],
            page_size: 7,
            ..Config::default()
        };
        let mut app = App::new(reglages);
        assert_eq!(
            app.start(),
            vec![Command::FetchList {
                generation: 1,
                query: "is:pr review-requested:@me sort:updated-desc".to_string(),
                page_size: 7,
            }]
        );
    }

    #[test]
    fn un_filtre_inconnu_des_reglages_traverse_la_requete_intact() {
        let reglages = Config {
            filters: vec!["involves:@me -is:draft".to_string()],
            ..Config::default()
        };
        let mut app = App::new(reglages);
        match &app.start()[0] {
            Command::FetchList { query, .. } => {
                assert_eq!(query, "is:pr involves:@me -is:draft sort:updated-desc")
            }
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn l_ordre_des_filtres_des_reglages_ne_change_pas_les_termes_ramenes() {
        let requete = |filtres: Vec<&str>| {
            let reglages = Config {
                filters: filtres.into_iter().map(str::to_string).collect(),
                ..Config::default()
            };
            match &App::new(reglages).start()[0] {
                Command::FetchList { query, .. } => query.clone(),
                autre => panic!("commande inattendue : {autre:?}"),
            }
        };

        let un = requete(vec!["author:@me", "is:open"]);
        let autre = requete(vec!["is:open", "author:@me"]);
        assert!(un.starts_with("is:pr "), "{un}");
        assert!(un.ends_with(" sort:updated-desc"), "{un}");

        let mut mots_un: Vec<&str> = un.split(' ').collect();
        let mut mots_autre: Vec<&str> = autre.split(' ').collect();
        mots_un.sort_unstable();
        mots_autre.sort_unstable();
        assert_eq!(mots_un, mots_autre);
    }

    #[test]
    fn les_fleches_deplacent_la_selection() {
        let mut app = app_garnie(vec![pr(1), pr(2), pr(3)]);
        assert_eq!(app.selected, 0);

        assert!(app.handle(Event::Key(Key::Down)).is_empty());
        assert_eq!(app.selected, 1);

        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 2);

        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn j_et_k_deplacent_la_selection_comme_les_fleches() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Char('j')));
        assert_eq!(app.selected, 1);
        app.handle(Event::Key(Key::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn la_selection_ne_deborde_pas_des_extremites() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);

        // En haut de liste, la flèche haut ne fait rien : pas de bouclage.
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 0);

        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 1, "la dernière ligne est un mur");
    }

    #[test]
    fn une_liste_vide_n_a_pas_de_selection() {
        let mut app = app_garnie(vec![]);
        assert!(app.selected_pr().is_none());
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 0, "aucune touche ne doit paniquer");
        assert!(app.selected_pr().is_none());
    }

    #[test]
    fn le_rafraichissement_suit_la_pr_selectionnee() {
        let mut app = app_garnie(vec![pr(1), pr(2), pr(3)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected_pr().map(|pr| pr.key.number), Some(2));

        // La 2 est passée en queue : la sélection la suit.
        rafraichir(&mut app, vec![pr(3), pr(1), pr(2)]);
        assert_eq!(app.selected, 2);
        assert_eq!(app.selected_pr().map(|pr| pr.key.number), Some(2));
    }

    #[test]
    fn deux_depots_de_meme_numero_ne_sont_pas_confondus() {
        let mut app = app_garnie(vec![pr_de("moi/un", 7), pr_de("moi/autre", 7)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(
            app.selected_pr().map(|pr| pr.key.repo.clone()),
            Some("moi/autre".to_string())
        );

        rafraichir(&mut app, vec![pr_de("moi/autre", 7), pr_de("moi/un", 7)]);
        assert_eq!(
            app.selected_pr().map(|pr| pr.key.repo.clone()),
            Some("moi/autre".to_string()),
            "la clé porte le dépôt, pas seulement le numéro"
        );
    }

    #[test]
    fn une_pr_disparue_laisse_la_selection_dans_les_bornes() {
        let mut app = app_garnie(vec![pr(1), pr(2), pr(3)]);
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 2);

        // La 3 a été fusionnée : la liste rétrécit.
        rafraichir(&mut app, vec![pr(1), pr(2)]);
        assert_eq!(
            app.selected, 1,
            "l'indice précédent, borné à la nouvelle taille"
        );
        assert!(app.selected_pr().is_some());
    }

    #[test]
    fn une_liste_devenue_vide_n_a_plus_de_selection() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        rafraichir(&mut app, vec![]);
        assert_eq!(app.selected, 0);
        assert!(app.selected_pr().is_none());
    }

    #[test]
    fn un_tick_pendant_un_chargement_de_liste_ne_relance_rien() {
        let (mut app, _) = app_demarree();
        assert!(app.loading.list, "la requête de démarrage est en cours");
        assert!(
            app.handle(Event::Tick).is_empty(),
            "aucune seconde requête tant que la première n'a pas répondu"
        );
    }

    #[test]
    fn un_tick_apres_la_reponse_relance_la_liste() {
        let mut app = app_garnie(vec![pr(1)]);
        assert!(!app.loading.list);
        match &app.handle(Event::Tick)[0] {
            Command::FetchList { .. } => {}
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn ctrl_c_quitte() {
        let (mut app, _) = app_demarree();
        assert_eq!(app.handle(Event::Key(Key::CtrlC)), vec![Command::Quit]);
        assert!(app.should_quit);
    }

    /// Détail d'une pull request, minimal mais complet dans sa forme.
    pub(crate) fn detail(numero: u32) -> PrDetail {
        let resume = pr(numero);
        PrDetail {
            node_id: format!("PR_{numero}"),
            body: "Première ligne.\nSeconde ligne.".to_string(),
            head_ref: "ma-branche".to_string(),
            base_ref: "develop".to_string(),
            checks: vec![CheckRun {
                name: "tests".to_string(),
                state: ChecksState::Success,
                url: None,
            }],
            reviews: vec![Review {
                author: "collegue".to_string(),
                state: ReviewState::Approved,
                body: "Ça me va.".to_string(),
                submitted_at: "2026-08-30T10:00:00Z".parse().expect("date valide"),
            }],
            comments: vec![Comment {
                author: "moi".to_string(),
                body: "Rebasé.".to_string(),
                created_at: "2026-08-30T11:00:00Z".parse().expect("date valide"),
            }],
            files: vec![
                ChangedFile {
                    path: "src/app/mod.rs".to_string(),
                    additions: 12,
                    deletions: 3,
                },
                // Chemin délibérément long : rend probante la troncature des
                // lignes de détail.
                ChangedFile {
                    path: "src/app/un/chemin/de/fichier/particulierement/long/pour/verifier/la/troncature.rs".to_string(),
                    additions: 1,
                    deletions: 0,
                },
            ],
            additions: 12,
            deletions: 3,
            summary: resume,
        }
    }

    /// Ouvre le détail de la sélection et rend la génération demandée.
    fn ouvrir_detail(app: &mut App) -> Generation {
        match &app.handle(Event::Key(Key::Right))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn la_fleche_droite_ouvre_le_detail_et_demande_les_donnees() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        let commandes = app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));
        match &commandes[..] {
            [Command::FetchDetail { summary, .. }] => assert_eq!(summary.key.number, 2),
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert!(app.loading.detail);
    }

    #[test]
    fn entree_ouvre_aussi_le_detail() {
        let mut app = app_garnie(vec![pr(1)]);
        app.handle(Event::Key(Key::Enter));
        assert!(matches!(app.view, View::Detail { .. }));
    }

    #[test]
    fn ouvrir_une_pr_deja_en_cache_n_emet_aucune_commande() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        app.handle(Event::Key(Key::Left));

        let commandes = app.handle(Event::Key(Key::Right));
        assert!(
            commandes.is_empty(),
            "le cache de la session évite la requête : {commandes:?}"
        );
        assert!(!app.loading.detail);
    }

    #[test]
    fn la_fleche_gauche_et_echap_reviennent_a_la_liste() {
        let mut app = app_garnie(vec![pr(1)]);
        ouvrir_detail(&mut app);
        assert!(app.handle(Event::Key(Key::Left)).is_empty());
        assert!(matches!(app.view, View::List));

        ouvrir_detail(&mut app);
        app.handle(Event::Key(Key::Esc));
        assert!(matches!(app.view, View::List));
    }

    #[test]
    fn une_liste_vide_n_ouvre_pas_de_detail() {
        let mut app = app_garnie(vec![]);
        assert!(app.handle(Event::Key(Key::Right)).is_empty());
        assert!(matches!(app.view, View::List));
    }

    #[test]
    fn r_en_vue_detail_recharge_le_detail_et_pas_la_liste() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });

        match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail {
                generation: neuve, ..
            }] => {
                assert!(*neuve > generation, "une nouvelle génération s'ouvre")
            }
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn les_fleches_font_defiler_le_detail_sans_deborder() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        assert_eq!(app.detail_scroll(), 0);

        // En haut, la flèche haut ne fait rien.
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.detail_scroll(), 0);

        app.handle(Event::Key(Key::Down));
        assert_eq!(app.detail_scroll(), 1);
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.detail_scroll(), 0);

        // Le bas de la zone est un mur : on ne défile pas dans le vide.
        for _ in 0..500 {
            app.handle(Event::Key(Key::Down));
        }
        let dernier = app.detail_scroll() as usize;
        assert!(dernier > 0);
        assert!(
            dernier < app.detail_lines(u16::MAX).len(),
            "défilement borné au contenu"
        );
    }

    #[test]
    fn un_detail_perime_est_ignore() {
        let mut app = app_garnie(vec![pr(1)]);
        let premiere = ouvrir_detail(&mut app);
        // Rechargement : la réponse lente de la première arrive après.
        app.handle(Event::Key(Key::Char('r')));
        app.handle(Event::DetailLoaded {
            generation: premiere,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        assert!(app.details.is_empty(), "la réponse périmée ne se range pas");
        assert!(app.loading.detail, "la requête en cours reste en cours");
    }

    #[test]
    fn ouvrir_un_detail_ne_perime_pas_une_requete_de_liste_en_vol() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        let generation_liste = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        ouvrir_detail(&mut app);

        app.handle(Event::ListLoaded {
            generation: generation_liste,
            result: Ok(page(vec![pr(1), pr(2), pr(3)])),
        });
        assert_eq!(app.prs.len(), 3, "le résultat de liste doit être accepté");
        assert!(!app.loading.list);
    }

    #[test]
    fn un_rafraichissement_de_liste_ne_vide_pas_le_cache_des_details() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        app.handle(Event::Key(Key::Left));

        rafraichir(&mut app, vec![pr(1)]);
        assert!(
            app.details.contains_key(&pr(1).key),
            "le compromis est assumé : le détail reste en cache jusqu'à r"
        );
    }

    #[test]
    fn une_erreur_de_detail_est_reprise_telle_quelle() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.error.as_deref(), Some("Réseau injoignable."));
        assert!(!app.loading.detail);
    }

    #[test]
    fn o_ouvre_la_pr_selectionnee_dans_le_navigateur() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/2".to_string()
            }]
        );
    }

    #[test]
    fn o_en_vue_detail_ouvre_la_pr_affichee() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        ouvrir_detail(&mut app);
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/2".to_string()
            }]
        );
    }

    /// Détail ouvert et chargé, puis PR retirée de la liste — fusionnée,
    /// fermée, ou sortie du filtre — alors que la vue reste affichée.
    fn app_en_detail_hors_liste() -> App {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        let generation_liste = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: generation_liste,
            result: Ok(page(vec![])),
        });
        assert!(app.prs.is_empty(), "la PR a bien quitté la liste");
        app
    }

    #[test]
    fn o_ouvre_encore_une_pr_qui_a_quitte_la_liste() {
        let mut app = app_en_detail_hors_liste();
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/1".to_string()
            }],
            "l'URL est dans le résumé porté par le détail en cache"
        );
    }

    #[test]
    fn r_recharge_encore_une_pr_qui_a_quitte_la_liste() {
        let mut app = app_en_detail_hors_liste();
        match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { summary, .. }] => assert_eq!(summary.key, pr(1).key),
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn un_detail_devenu_plus_court_reborne_le_defilement() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        for _ in 0..500 {
            app.handle(Event::Key(Key::Down));
        }
        let bas = app.detail_scroll();
        assert!(bas > 0, "le défilement est bien descendu");

        // Rechargement : la description et les échanges ont disparu, le
        // contenu est nettement plus court que le défilement en cours.
        let generation = match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(PrDetail {
                body: String::new(),
                checks: Vec::new(),
                reviews: Vec::new(),
                comments: Vec::new(),
                files: Vec::new(),
                ..detail(1)
            }),
        });

        assert!(app.detail_scroll() < bas, "le défilement a été ramené");
        assert!(
            (app.detail_scroll() as usize) < app.detail_line_count(),
            "sinon l'écran reste vide jusqu'à ce qu'on remonte : défilement \
             {} pour {} lignes",
            app.detail_scroll(),
            app.detail_line_count()
        );
    }

    #[test]
    fn o_sur_une_liste_vide_ne_fait_rien() {
        let mut app = app_garnie(vec![]);
        assert!(app.handle(Event::Key(Key::Char('o'))).is_empty());
    }

    #[test]
    fn m_est_reconnue_et_reste_sans_effet_jusqu_a_la_spec_04() {
        let mut app = app_garnie(vec![pr(1)]);
        let commandes = app.handle(Event::Key(Key::Char('m')));
        assert!(commandes.is_empty(), "commandes = {commandes:?}");
        assert!(matches!(app.view, View::List), "aucun changement de vue");
        assert!(app.error.is_none(), "et aucun message d'erreur");
    }
}
