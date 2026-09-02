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
use crossterm::event::{Event as TerminalEvent, KeyCode, KeyEventKind};
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
        let _ = restore_terminal();
    }
}

type Ecran = Terminal<CrosstermBackend<Stdout>>;

/// Prend le contrôle du terminal et installe le crochet de panique.
/// Le garde couvre la sortie normale et l'erreur, le crochet couvre la panique.
fn enter_terminal() -> Result<(Ecran, TerminalGuard)> {
    let crochet_precedent = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |infos| {
        let _ = restore_terminal();
        crochet_precedent(infos);
    }));

    enable_raw_mode()?;
    let mut sortie = io::stdout();
    execute!(sortie, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(sortie))?;
    Ok((terminal, TerminalGuard))
}

/// Rend le terminal à l'utilisateur. Volontairement tolérante aux erreurs :
/// elle est appelée depuis un `Drop` et depuis un crochet de panique.
fn restore_terminal() -> Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    Ok(())
}

async fn run(reglages: config::Config, jeton: token::Token) -> Result<()> {
    let intervalle = reglages.refresh_interval;
    let jeton = Arc::new(jeton);
    let mut etat = App::new(reglages);

    let (envoi, mut reception) = mpsc::unbounded_channel::<Event>();

    // Producteur 1 : le clavier, dans une tâche bloquante dédiée.
    spawn_keyboard(envoi.clone());

    // Producteur 2 : le minuteur de rafraîchissement, si activé.
    if intervalle > 0 {
        spawn_timer(envoi.clone(), intervalle);
    }

    let (mut terminal, _garde) = enter_terminal()?;

    // Producteur 3 : les résultats réseau, une tâche par demande.
    for commande in etat.start() {
        execute_command(commande, &envoi, &jeton);
    }
    terminal.draw(|cadre| ui::draw(cadre, &etat))?;

    while let Some(evenement) = reception.recv().await {
        for commande in etat.handle(evenement) {
            execute_command(commande, &envoi, &jeton);
        }
        if etat.should_quit {
            break;
        }
        terminal.draw(|cadre| ui::draw(cadre, &etat))?;
    }

    Ok(())
}

/// Exécute une commande émise par `app`. C'est le seul endroit où le jeton
/// circule : il n'entre jamais dans `app` ni dans `ui`.
fn execute_command(commande: Command, envoi: &UnboundedSender<Event>, jeton: &Arc<token::Token>) {
    match commande {
        Command::Quit => {}
        Command::Fetch {
            generation,
            filters,
            page_size,
        } => {
            let envoi = envoi.clone();
            let jeton = jeton.clone();
            tokio::spawn(async move {
                let resultat =
                    github::fetch_pull_requests(jeton.expose(), &filters, page_size).await;
                let _ = envoi.send(Event::Data {
                    generation,
                    result: resultat,
                });
            });
        }
    }
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
            Err(_) => return,
        }

        let Ok(TerminalEvent::Key(touche)) = crossterm::event::read() else {
            continue;
        };
        if touche.kind != KeyEventKind::Press {
            continue;
        }
        let traduite = match touche.code {
            KeyCode::Char(caractere) => Key::Char(caractere),
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
