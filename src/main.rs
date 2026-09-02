//! Démarrage de `owl` : réglages, jeton, écran, boucle d'événements.

mod app;
mod config;
mod filter;
mod github;
mod model;
mod token;
mod ui;

use std::io::{self, Stdout};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event as TerminalEvent, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::app::{App, Command, Event, Key};

fn main() -> ExitCode {
    // Les erreurs de démarrage sont écrites avant toute prise de contrôle du
    // terminal, donc jamais avalées par l'écran alterné.
    let reglages = match config::load() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    let jeton = match token::resolve() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    // L'exécuteur n'est construit qu'après les vérifications de démarrage.
    let execution = match tokio::runtime::Runtime::new() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            eprintln!("{erreur}");
            return ExitCode::FAILURE;
        }
    };

    match execution.block_on(run(reglages, jeton)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erreur) => {
            eprintln!("{erreur}");
            ExitCode::FAILURE
        }
    }
}

/// Restaure le terminal à la sortie de portée, quelle qu'en soit la cause.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

type Ecran = Terminal<CrosstermBackend<Stdout>>;

/// Prend le contrôle du terminal et installe le crochet de panique.
/// Le garde couvre la sortie normale et l'erreur, le crochet couvre la panique.
///
/// Le crochet reçoit une copie de l'émetteur d'événements : une panique dans
/// une tâche n'arrête que cette tâche, alors que le crochet rend déjà le
/// terminal. Sans un `Quit` poussé dans la file, la boucle continuerait à
/// dessiner par-dessus le shell rendu à l'utilisateur.
fn enter_terminal(envoi: UnboundedSender<Event>) -> Result<(Ecran, TerminalGuard)> {
    let crochet_precedent = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |infos| {
        restore_terminal();
        let _ = envoi.send(Event::Quit);
        crochet_precedent(infos);
    }));

    enable_raw_mode()?;
    // Le garde naît dès la première prise de contrôle réussie : toute erreur
    // rencontrée ensuite rend malgré tout le terminal en sortant de portée.
    let garde = TerminalGuard;
    let mut sortie = io::stdout();
    execute!(sortie, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(sortie))?;
    Ok((terminal, garde))
}

/// Rend le terminal à l'utilisateur. Volontairement tolérante aux erreurs :
/// elle est appelée depuis un `Drop` et depuis un crochet de panique.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

async fn run(reglages: config::Config, jeton: token::Token) -> Result<()> {
    let intervalle = reglages.refresh_interval;
    // Le client est construit une fois pour toutes : il porte le jeton dans
    // ses en-têtes, et c'est le seul endroit du programme où le jeton reste.
    let client = Arc::new(github::Client::new(jeton.expose())?);
    let mut etat = App::new(reglages);

    let (envoi, mut reception) = mpsc::unbounded_channel::<Event>();

    // L'écran est pris avant de lancer les producteurs : le clavier doit lire
    // un terminal en mode brut, jamais un terminal encore en mode ligne.
    let (mut terminal, _garde) = enter_terminal(envoi.clone())?;

    // Producteur 1 : le clavier, dans une tâche bloquante dédiée.
    spawn_keyboard(envoi.clone());

    // Producteur 2 : le minuteur de rafraîchissement, si activé.
    if intervalle > 0 {
        spawn_timer(envoi.clone(), intervalle);
    }

    // Producteur 3 : les résultats réseau, une tâche par demande.
    // `start` ne demande jamais l'arrêt : son résultat n'a rien à décider.
    for commande in etat.start() {
        execute_command(commande, &envoi, &client);
    }
    terminal.draw(|cadre| ui::draw(cadre, &etat))?;

    while let Some(evenement) = reception.recv().await {
        let mut arret = false;
        for commande in etat.handle(evenement) {
            arret |= execute_command(commande, &envoi, &client);
        }
        if arret {
            break;
        }
        terminal.draw(|cadre| ui::draw(cadre, &etat))?;
    }

    Ok(())
}

/// Exécute une commande émise par `app` et rend `true` si elle demande
/// l'arrêt de la boucle. C'est le seul endroit où le jeton circule : il
/// n'entre jamais dans `app` ni dans `ui`.
fn execute_command(
    commande: Command,
    envoi: &UnboundedSender<Event>,
    client: &Arc<github::Client>,
) -> bool {
    match commande {
        Command::Quit => return true,
        Command::FetchList {
            generation,
            query,
            page_size,
        } => {
            let envoi = envoi.clone();
            let client = client.clone();
            tokio::spawn(async move {
                let resultat = client.fetch_pull_requests(&query, page_size).await;
                let _ = envoi.send(Event::ListLoaded {
                    generation,
                    result: resultat,
                });
            });
        }
        Command::FetchDetail {
            generation,
            summary,
        } => {
            let envoi = envoi.clone();
            let client = client.clone();
            let cle = summary.key.clone();
            tokio::spawn(async move {
                let resultat = client.fetch_detail(&summary).await;
                let _ = envoi.send(Event::DetailLoaded {
                    generation,
                    key: cle,
                    result: resultat,
                });
            });
        }
    }
    false
}

/// Lit le clavier dans une tâche bloquante et traduit les touches pour `app`.
/// La traduction est faite ici pour que `app` ne dépende pas de `crossterm`.
fn spawn_keyboard(envoi: UnboundedSender<Event>) {
    tokio::task::spawn_blocking(move || loop {
        // Le sondage évite de bloquer indéfiniment sur un canal fermé.
        match crossterm::event::poll(Duration::from_millis(200)) {
            Ok(true) => {}
            Ok(false) => {
                if envoi.is_closed() {
                    return;
                }
                continue;
            }
            // Clavier hors service : sans touche, plus aucun `q` ne peut
            // arriver, et le mode brut a désarmé Ctrl-C. On demande l'arrêt
            // plutôt que de laisser le programme figé.
            Err(_) => {
                let _ = envoi.send(Event::Quit);
                return;
            }
        }

        let Ok(TerminalEvent::Key(touche)) = crossterm::event::read() else {
            continue;
        };
        if touche.kind != KeyEventKind::Press {
            continue;
        }
        let traduite = match (touche.code, touche.modifiers) {
            // Ctrl+C d'abord : sans ce cas, elle passerait pour un « c ».
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Key::CtrlC,
            (KeyCode::Char(caractere), _) => Key::Char(caractere),
            (KeyCode::Up, _) => Key::Up,
            (KeyCode::Down, _) => Key::Down,
            (KeyCode::Left, _) => Key::Left,
            (KeyCode::Right, _) => Key::Right,
            (KeyCode::Enter, _) => Key::Enter,
            (KeyCode::Esc, _) => Key::Esc,
            _ => Key::Other,
        };
        if envoi.send(Event::Key(traduite)).is_err() {
            return;
        }
    });
}

/// Émet un `Tick` à intervalle régulier.
fn spawn_timer(envoi: UnboundedSender<Event>, secondes: u64) {
    tokio::spawn(async move {
        let mut minuteur = tokio::time::interval(Duration::from_secs(secondes));
        // Une boucle ralentie ne doit pas rattraper les tours manqués : sinon
        // chaque retard déclencherait une rafale de requêtes.
        minuteur.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Le premier tour part immédiatement : on le consomme, `start` a déjà
        // lancé la requête initiale.
        minuteur.tick().await;
        loop {
            minuteur.tick().await;
            if envoi.send(Event::Tick).is_err() {
                return;
            }
        }
    });
}
