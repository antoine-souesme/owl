//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

use chrono::{DateTime, Local};

use crate::config::Config;
use crate::filter::{self, Filter};
use crate::github::GithubError;
use crate::model::{ListPage, PrSummary};

/// Numéro de génération d'une demande réseau. Un résultat dont la génération
/// est périmée est ignoré, ce qui évite qu'une réponse lente écrase une
/// réponse plus récente.
pub type Generation = u64;

/// Touche reçue, traduite par `main`. `app` ignore volontairement `crossterm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    /// Toute autre touche, sans effet au stade des fondations.
    Other,
}

/// Ce qui arrive dans la file d'événements.
#[derive(Debug)]
pub enum Event {
    Key(Key),
    /// Tour de minuteur de rafraîchissement.
    Tick,
    /// Arrêt demandé par `main` : panique d'une tâche, ou clavier hors service.
    /// Sans lui, la boucle resterait bloquée sur la file d'événements.
    Quit,
    /// Résultat d'une demande réseau.
    Data {
        generation: Generation,
        result: Result<ListPage, GithubError>,
    },
}

/// Aide clavier, en fin de barre d'état. Le texte est ici, pas dans `ui`.
const AIDE: &str = "q quitter · r rafraîchir";

/// Message affiché tant qu'aucune réponse n'est arrivée.
const ATTENTE_INITIALE: &str = "Chargement…";

/// Ce que `app` demande à `main` de faire.
#[derive(Debug, PartialEq)]
pub enum Command {
    Fetch {
        generation: Generation,
        /// Chaîne de recherche complète, assemblée par `filter::build_query`.
        query: String,
        page_size: u16,
    },
    Quit,
}

pub struct App {
    pub items: Vec<PrSummary>,
    /// Message principal de la barre d'état : le résumé de la liste, ou
    /// l'erreur en cours. Vide tant qu'aucune réponse n'est arrivée.
    pub status: String,
    pub loading: bool,
    pub should_quit: bool,
    pub last_refresh: Option<DateTime<Local>>,
    /// Solde d'appels rapporté par la dernière requête réussie. Conservé ici
    /// parce que la spec 01 le demande ; la suspension du rafraîchissement
    /// qu'il déclenche appartient à `05-erreurs-et-tests.md`.
    #[allow(dead_code)]
    pub rate_limit: Option<crate::model::RateLimit>,
    generation: Generation,
    /// Filtres des réglages, traduits une seule fois. Un futur changement de
    /// filtre depuis l'écran n'aura que ce vecteur à modifier.
    filters: Vec<Filter>,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            items: Vec::new(),
            status: String::new(),
            loading: false,
            should_quit: false,
            last_refresh: None,
            rate_limit: None,
            generation: 0,
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
        vec![self.fetch()]
    }

    pub fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Key(Key::Char('q')) => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            Event::Quit => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            Event::Key(Key::Char('r')) | Event::Tick => vec![self.fetch()],
            Event::Key(_) => Vec::new(),
            Event::Data { generation, result } => {
                // Réponse d'une demande dépassée : on la jette sans rien changer.
                if generation != self.generation {
                    return Vec::new();
                }
                self.loading = false;
                match result {
                    Ok(page) => {
                        self.items = page.pull_requests;
                        self.rate_limit = page.rate_limit;
                        self.last_refresh = Some(Local::now());
                        self.status = self.liste_resumee();
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(erreur) => self.status = erreur.to_string(),
                }
                Vec::new()
            }
        }
    }

    /// Ouvre une nouvelle génération et demande les données.
    fn fetch(&mut self) -> Command {
        self.generation += 1;
        self.loading = true;
        Command::Fetch {
            generation: self.generation,
            query: filter::build_query(&self.filters),
            page_size: self.config.page_size,
        }
    }

    /// Barre d'état complète, prête à dessiner telle quelle.
    ///
    /// Assemblée ici, et pas dans `ui`, parce que chaque morceau est une
    /// décision : le libellé de l'heure, l'annonce d'une requête en cours,
    /// l'aide clavier. `ui` se contente d'afficher la chaîne rendue.
    pub fn status_line(&self) -> String {
        let mut morceaux: Vec<String> = Vec::new();

        if self.status.is_empty() {
            // Rien n'est encore arrivé : l'attente est le message principal,
            // et il serait redondant de l'annoncer une seconde fois.
            morceaux.push(ATTENTE_INITIALE.to_string());
        } else {
            morceaux.push(self.status.clone());
            if let Some(instant) = self.last_refresh {
                morceaux.push(format!("mis à jour à {}", instant.format("%H:%M")));
            }
            if self.loading {
                morceaux.push("chargement…".to_string());
            }
        }

        morceaux.push(AIDE.to_string());
        morceaux.join(" · ")
    }

    /// Résumé de la liste pour la barre d'état.
    fn liste_resumee(&self) -> String {
        match self.items.len() {
            0 => "Aucune pull request".to_string(),
            1 => "1 pull request".to_string(),
            nombre => format!("{nombre} pull requests"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::model::{
        ChecksState, ListPage, MergeableState, PrKey, PrSummary, RepoMergeRules, ReviewState,
    };

    fn pr(numero: u32) -> PrSummary {
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
    fn page(pull_requests: Vec<PrSummary>) -> ListPage {
        ListPage {
            pull_requests,
            rate_limit: None,
        }
    }

    /// Application démarrée, première requête émise, génération courante rendue.
    fn app_demarree() -> (App, Generation) {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        let generation = match &commandes[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        (app, generation)
    }

    #[test]
    fn le_demarrage_emet_une_seule_requete() {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        assert_eq!(
            commandes,
            vec![Command::Fetch {
                generation: 1,
                query: "is:pr author:@me is:open sort:updated-desc".to_string(),
                page_size: 50,
            }]
        );
        assert!(app.loading);
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
            Command::Fetch { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert!(app.loading);
    }

    #[test]
    fn le_minuteur_relance_une_requete() {
        let (mut app, premiere) = app_demarree();
        let commandes = app.handle(Event::Tick);
        match &commandes[0] {
            Command::Fetch { generation, .. } => assert!(*generation > premiere),
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
        let commandes = app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(commandes.is_empty());
        assert_eq!(app.items, vec![pr(1), pr(2)]);
        assert!(!app.loading);
        assert!(app.last_refresh.is_some());
    }

    #[test]
    fn un_resultat_perime_est_ignore() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        // Une nouvelle requête part, puis la réponse lente de l'ancienne arrive.
        app.handle(Event::Key(Key::Char('r')));
        let commandes = app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(99)])),
        });
        assert!(commandes.is_empty());
        assert_eq!(
            app.items,
            vec![pr(1)],
            "la réponse lente ne doit rien écraser"
        );
        assert!(app.loading, "la requête en cours reste en cours");
    }

    #[test]
    fn une_erreur_laisse_la_liste_affichee() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.items, vec![pr(1)], "la liste précédente reste visible");
        assert_eq!(app.status, "Réseau injoignable.", "message repris tel quel");
        assert!(!app.loading);
    }

    #[test]
    fn un_succes_efface_l_erreur_en_cours() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Err(GithubError::Transport),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(
            !app.status.contains("Réseau injoignable"),
            "status = {}",
            app.status
        );
    }

    #[test]
    fn le_status_annonce_le_nombre_de_pull_requests() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(app.status.starts_with("2 pull requests"), "{}", app.status);

        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![])),
        });
        assert_eq!(app.status, "Aucune pull request");
    }

    /// Au stade des fondations, un tour de minuteur relance sans condition et
    /// la réponse abandonnée est jetée par sa génération.
    /// `03-affichage-et-navigation.md` resserrera cette règle : un `Tick` reçu
    /// pendant un chargement n'émettra plus de seconde requête.
    #[test]
    fn un_tick_pendant_un_chargement_relance_et_jette_la_premiere_reponse() {
        let (mut app, premiere) = app_demarree();
        let commandes = app.handle(Event::Tick);
        let seconde = match &commandes[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        assert!(seconde > premiere, "une nouvelle génération doit s'ouvrir");
        assert!(app.loading);

        // La réponse de la requête abandonnée arrive après : elle est jetée.
        app.handle(Event::Data {
            generation: premiere,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(app.items.is_empty(), "la première réponse doit être jetée");
        assert!(app.loading, "la seconde requête reste en cours");

        app.handle(Event::Data {
            generation: seconde,
            result: Ok(page(vec![pr(2)])),
        });
        assert_eq!(app.items, vec![pr(2)]);
        assert!(!app.loading);
    }

    #[test]
    fn q_pendant_un_chargement_quitte_quand_meme() {
        let (mut app, _) = app_demarree();
        assert!(app.loading, "une requête est bien en cours");
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
        assert_eq!(app.status_line(), "Chargement… · q quitter · r rafraîchir");
    }

    #[test]
    fn la_barre_d_etat_apres_une_reponse_donne_l_heure_et_l_aide() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(),
            format!("2 pull requests · mis à jour à {heure} · q quitter · r rafraîchir")
        );
    }

    #[test]
    fn la_barre_d_etat_annonce_le_chargement_d_un_rafraichissement() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        app.handle(Event::Key(Key::Char('r')));
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(),
            format!(
                "1 pull request · mis à jour à {heure} · chargement… · q quitter · r rafraîchir"
            )
        );
    }

    #[test]
    fn la_barre_d_etat_reprend_l_erreur_telle_quelle() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(),
            "Réseau injoignable. · q quitter · r rafraîchir",
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
            vec![Command::Fetch {
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
            Command::Fetch { query, .. } => {
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
                Command::Fetch { query, .. } => query.clone(),
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
}
