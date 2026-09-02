//! Démarrage de `owl` : réglages, jeton, écran, restauration du terminal.

mod config;
mod token;

use std::io::{self, Stdout};
use std::process::ExitCode;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

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

    match run(reglages, jeton) {
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
/// Le garde et le crochet font le même travail : le garde couvre la sortie
/// normale et l'erreur, le crochet couvre la panique.
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

fn run(reglages: config::Config, _jeton: token::Token) -> Result<()> {
    let (mut terminal, _garde) = enter_terminal()?;

    loop {
        terminal.draw(|cadre| {
            let contenu = Paragraph::new(format!(
                "{} filtres actifs — « q » pour quitter",
                reglages.filters.len()
            ))
            .block(Block::default().borders(Borders::ALL).title(" owl "));
            cadre.render_widget(contenu, cadre.area());
        })?;

        if let Event::Key(touche) = event::read()? {
            if touche.kind == KeyEventKind::Press && touche.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    Ok(())
}
