//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

use chrono::{DateTime, Local};

use crate::config::Config;
use crate::model::PullRequest;

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
    /// Résultat d'une demande réseau.
    Data {
        generation: Generation,
        result: Result<Vec<PullRequest>, String>,
    },
}

/// Ce que `app` demande à `main` de faire.
#[derive(Debug, PartialEq)]
pub enum Command {
    Fetch {
        generation: Generation,
        filters: Vec<String>,
        page_size: u16,
    },
    Quit,
}

pub struct App {
    pub items: Vec<PullRequest>,
    /// Ligne affichée dans la barre d'état, prête à dessiner.
    pub status: String,
    pub loading: bool,
    pub should_quit: bool,
    pub last_refresh: Option<DateTime<Local>>,
    generation: Generation,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            items: Vec::new(),
            status: "Chargement…".to_string(),
            loading: false,
            should_quit: false,
            last_refresh: None,
            generation: 0,
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
            Event::Key(Key::Char('r')) | Event::Tick => vec![self.fetch()],
            Event::Key(_) => Vec::new(),
            Event::Data { generation, result } => {
                // Réponse d'une demande dépassée : on la jette sans rien changer.
                if generation != self.generation {
                    return Vec::new();
                }
                self.loading = false;
                match result {
                    Ok(items) => {
                        self.items = items;
                        self.last_refresh = Some(Local::now());
                        self.status = self.liste_resumee();
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(message) => self.status = message,
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
            filters: self.config.filters.clone(),
            page_size: self.config.page_size,
        }
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

    fn pr(numero: u32) -> PullRequest {
        PullRequest {
            repository: "moi/depot".to_string(),
            number: numero,
            title: format!("Titre {numero}"),
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
                filters: vec!["author:@me".to_string(), "is:open".to_string()],
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
            result: Ok(vec![pr(1), pr(2)]),
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
            result: Ok(vec![pr(1)]),
        });
        // Une nouvelle requête part, puis la réponse lente de l'ancienne arrive.
        app.handle(Event::Key(Key::Char('r')));
        let commandes = app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(99)]),
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
            result: Ok(vec![pr(1)]),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Err("Réseau injoignable".to_string()),
        });
        assert_eq!(app.items, vec![pr(1)], "la liste précédente reste visible");
        assert_eq!(app.status, "Réseau injoignable", "message repris tel quel");
        assert!(!app.loading);
    }

    #[test]
    fn un_succes_efface_l_erreur_en_cours() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::Data {
            generation,
            result: Err("Réseau injoignable".to_string()),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Ok(vec![pr(1)]),
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
            result: Ok(vec![pr(1), pr(2)]),
        });
        assert!(app.status.starts_with("2 pull requests"), "{}", app.status);

        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::Fetch { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::Data {
            generation,
            result: Ok(vec![]),
        });
        assert_eq!(app.status, "Aucune pull request");
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
                filters: vec!["review-requested:@me".to_string()],
                page_size: 7,
            }]
        );
    }
}
