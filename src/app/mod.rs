//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

mod render;

// `Glyph` n'est pas encore consommé hors de `app` : `ui/list.rs` accède à ses
// champs sans nommer le type. La tâche 3 l'utilisera pour `DetailLine`.
#[allow(unused_imports)]
pub use render::{Glyph, ListRender, ListRow, Tone};

use chrono::{DateTime, Local};

use crate::config::Config;
use crate::filter::{self, Filter};
use crate::github::GithubError;
use crate::model::{ListPage, PrKey, PrSummary, RateLimit};

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
#[derive(Debug)]
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
const AIDE: &str = "↑↓ naviguer · → détail · m fusionner · r rafraîchir · o navigateur · q quitter";

/// Message affiché tant qu'aucune réponse n'est arrivée.
const ATTENTE_INITIALE: &str = "Chargement…";

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
    list_generation: Generation,
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
            list_generation: 0,
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
        }
    }

    fn handle_key(&mut self, touche: Key) -> Vec<Command> {
        match touche {
            Key::Char('q') | Key::CtrlC => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            Key::Char('r') => vec![self.fetch_list()],
            Key::Up | Key::Char('k') => {
                self.select_previous();
                Vec::new()
            }
            Key::Down | Key::Char('j') => {
                self.select_next();
                Vec::new()
            }
            _ => Vec::new(),
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

    /// Barre d'état complète, prête à dessiner telle quelle.
    ///
    /// Assemblée ici, et pas dans `ui`, parce que chaque morceau est une
    /// décision : le libellé de l'heure, l'annonce d'une requête en cours,
    /// l'aide clavier.
    pub fn status_line(&self) -> String {
        let mut morceaux: Vec<String> = Vec::new();

        if self.last_refresh.is_none() && self.error.is_none() {
            // Rien n'est encore arrivé : l'attente est le message principal.
            morceaux.push(ATTENTE_INITIALE.to_string());
        } else {
            if self.last_refresh.is_some() {
                morceaux.push(self.liste_resumee());
            }
            if let Some(instant) = self.last_refresh {
                morceaux.push(format!("mis à jour à {}", instant.format("%H:%M")));
            }
            if self.loading.list || self.loading.detail {
                morceaux.push("chargement…".to_string());
            }
            if let Some(erreur) = &self.error {
                morceaux.push(erreur.clone());
            }
        }

        morceaux.push(AIDE.to_string());
        morceaux.join(" · ")
    }

    /// Résumé de la liste pour la barre d'état.
    fn liste_resumee(&self) -> String {
        match self.prs.len() {
            0 => "Aucune pull request".to_string(),
            1 => "1 pull request".to_string(),
            nombre => format!("{nombre} pull requests"),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use crate::model::{
        ChecksState, ListPage, MergeableState, PrKey, PrSummary, RepoMergeRules, ReviewState,
    };

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
            app.status_line().starts_with("2 pull requests"),
            "{}",
            app.status_line()
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
            app.status_line().starts_with("Aucune pull request"),
            "{}",
            app.status_line()
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
        assert_eq!(app.status_line(), format!("Chargement… · {AIDE}"));
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
            app.status_line(),
            format!("2 pull requests · mis à jour à {heure} · {AIDE}")
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
            app.status_line(),
            format!("1 pull request · mis à jour à {heure} · chargement… · {AIDE}")
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
            app.status_line(),
            format!("Réseau injoignable. · {AIDE}"),
            "aucune heure : aucun rafraîchissement n'a encore réussi"
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
}
