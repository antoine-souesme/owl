//! Démarrage de `owl` : réglages, jeton, écran, boucle d'événements.

mod app;
mod config;
mod filter;
mod github;
mod model;
mod startup;
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
    let settings = match config::load() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let token = match token::resolve() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    // L'exécuteur n'est construit qu'après les vérifications de démarrage.
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(settings, token)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
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

type Screen = Terminal<CrosstermBackend<Stdout>>;

/// Prend le contrôle du terminal et installe le crochet de panique.
/// Le garde couvre la sortie normale et l'erreur, le crochet couvre la panique.
///
/// Le crochet reçoit une copie de l'émetteur d'événements : une panique dans
/// une tâche n'arrête que cette tâche, alors que le crochet rend déjà le
/// terminal. Sans un `Quit` poussé dans la file, la boucle continuerait à
/// dessiner par-dessus le shell rendu à l'utilisateur.
fn enter_terminal(sender: UnboundedSender<Event>) -> Result<(Screen, TerminalGuard)> {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |infos| {
        restore_terminal();
        let _ = sender.send(Event::Quit);
        previous_hook(infos);
    }));

    enable_raw_mode()?;
    // Le garde naît dès la première prise de contrôle réussie : toute erreur
    // rencontrée ensuite rend malgré tout le terminal en sortant de portée.
    let guard = TerminalGuard;
    let mut output = io::stdout();
    execute!(output, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(output))?;
    Ok((terminal, guard))
}

/// Rend le terminal à l'utilisateur. Volontairement tolérante aux erreurs :
/// elle est appelée depuis un `Drop` et depuis un crochet de panique.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

async fn run(settings: config::Config, token: token::Token) -> Result<()> {
    let interval = settings.refresh_interval;
    // Le client est construit une fois pour toutes : il porte le jeton dans
    // ses en-têtes, et c'est le seul endroit du programme où le jeton reste.
    let client = Arc::new(github::Client::new(token.expose())?);
    let mut state = App::new(settings);

    // La première requête part avant l'écran : un jeton refusé ou des droits
    // insuffisants sont des erreurs de démarrage, et leur message doit sortir
    // sur la sortie d'erreur, pas finir en ligne de barre d'état.
    let first_events = first_request(&mut state, &client).await?;

    let (sender, mut inbox) = mpsc::unbounded_channel::<Event>();

    // Le résultat déjà obtenu entre dans la file avant tout le reste : la
    // boucle le traitera à son premier tour.
    for event in first_events {
        let _ = sender.send(event);
    }

    // L'écran est pris avant de lancer les producteurs : le clavier doit lire
    // un terminal en mode brut, jamais un terminal encore en mode ligne.
    let (mut terminal, _guard) = enter_terminal(sender.clone())?;

    // Producteur 1 : le clavier, dans une tâche bloquante dédiée.
    spawn_keyboard(sender.clone());

    // Producteur 2 : le minuteur de rafraîchissement, si activé.
    if interval > 0 {
        spawn_timer(sender.clone(), interval);
    }

    terminal.draw(|frame| ui::draw(frame, &state))?;

    while let Some(event) = inbox.recv().await {
        let mut stop = false;
        for command in state.handle(event) {
            stop |= execute_command(command, &sender, &client);
        }
        if stop {
            break;
        }
        terminal.draw(|frame| ui::draw(frame, &state))?;
    }

    Ok(())
}

/// Exécute la demande initiale de `app` et rend les événements à injecter
/// dans la boucle. Une erreur de démarrage remonte en `Err` : `main` l'écrit
/// et s'arrête, le terminal n'ayant jamais été pris.
async fn first_request(state: &mut App, client: &Arc<github::Client>) -> Result<Vec<Event>> {
    let mut evenements = Vec::new();
    for command in state.start() {
        match command {
            Command::FetchList {
                generation,
                query,
                page_size,
            } => {
                let result = client.fetch_pull_requests(&query, page_size).await;
                match startup::classify(result) {
                    startup::FirstResponse::Fatal(message) => return Err(anyhow::anyhow!(message)),
                    startup::FirstResponse::Start(result) => {
                        evenements.push(Event::ListLoaded { generation, result })
                    }
                }
            }
            // `start` n'émet que la demande de liste. Toute autre commande
            // serait un changement de `app` non répercuté ici.
            other => unreachable!("commande inattendue au démarrage : {other:?}"),
        }
    }
    Ok(evenements)
}

/// Exécute une commande émise par `app` et rend `true` si elle demande
/// l'arrêt de la boucle. C'est le seul endroit où le jeton circule : il
/// n'entre jamais dans `app` ni dans `ui`.
fn execute_command(
    command: Command,
    sender: &UnboundedSender<Event>,
    client: &Arc<github::Client>,
) -> bool {
    match command {
        Command::Quit => return true,
        Command::FetchList {
            generation,
            query,
            page_size,
        } => {
            let sender = sender.clone();
            let client = client.clone();
            tokio::spawn(async move {
                let result = client.fetch_pull_requests(&query, page_size).await;
                let _ = sender.send(Event::ListLoaded { generation, result });
            });
        }
        Command::FetchDetail {
            generation,
            summary,
        } => {
            let sender = sender.clone();
            let client = client.clone();
            let key = summary.key.clone();
            tokio::spawn(async move {
                let result = client.fetch_detail(&summary).await;
                let _ = sender.send(Event::DetailLoaded {
                    generation,
                    key,
                    result,
                });
            });
        }
        Command::Merge {
            summary,
            node_id,
            method,
        } => {
            let sender = sender.clone();
            let client = client.clone();
            let key = summary.key.clone();
            tokio::spawn(async move {
                let result = client.merge_pull_request(&summary, node_id, method).await;
                let _ = sender.send(Event::MergeFinished { key, result });
            });
        }
        Command::OpenInBrowser { url } => {
            // Dans une tâche bloquante : lancer le navigateur peut prendre un
            // instant, et l'écran doit rester réactif pendant ce temps.
            // Un échec reste silencieux : aucune spec ne définit de message
            // pour ce cas.
            tokio::task::spawn_blocking(move || {
                let _ = open::that_detached(&url);
            });
        }
    }
    false
}

/// Lit le clavier dans une tâche bloquante et traduit les touches pour `app`.
/// La traduction est faite ici pour que `app` ne dépende pas de `crossterm`.
fn spawn_keyboard(sender: UnboundedSender<Event>) {
    tokio::task::spawn_blocking(move || loop {
        // Le sondage évite de bloquer indéfiniment sur un canal fermé.
        match crossterm::event::poll(Duration::from_millis(200)) {
            Ok(true) => {}
            Ok(false) => {
                if sender.is_closed() {
                    return;
                }
                continue;
            }
            // Clavier hors service : sans touche, plus aucun `q` ne peut
            // arriver, et le mode brut a désarmé Ctrl-C. On demande l'arrêt
            // plutôt que de laisser le programme figé.
            Err(_) => {
                let _ = sender.send(Event::Quit);
                return;
            }
        }

        let key_pressed = match crossterm::event::read() {
            Ok(TerminalEvent::Key(key_pressed)) => key_pressed,
            // Le redimensionnement remonte à `app` pour que la boucle
            // redessine : sans lui, l'écran reste figé à l'ancienne taille
            // jusqu'à la touche ou le tour de minuteur suivant.
            Ok(TerminalEvent::Resize(..)) => {
                if sender.send(Event::Resize).is_err() {
                    return;
                }
                continue;
            }
            _ => continue,
        };
        if key_pressed.kind != KeyEventKind::Press {
            continue;
        }
        let traduite = match (key_pressed.code, key_pressed.modifiers) {
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
        if sender.send(Event::Key(traduite)).is_err() {
            return;
        }
    });
}

/// Émet un `Tick` à intervalle régulier.
fn spawn_timer(sender: UnboundedSender<Event>, seconds: u64) {
    tokio::spawn(async move {
        let mut minuteur = tokio::time::interval(Duration::from_secs(seconds));
        // Une boucle ralentie ne doit pas rattraper les tours manqués : sinon
        // chaque retard déclencherait une rafale de requêtes.
        minuteur.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Le premier tour part immédiatement : on le consomme, `start` a déjà
        // lancé la requête initiale.
        minuteur.tick().await;
        loop {
            minuteur.tick().await;
            if sender.send(Event::Tick).is_err() {
                return;
            }
        }
    });
}
