mod config;
mod token;

fn main() {
    match (config::load(), token::resolve()) {
        (Ok(reglages), Ok(_)) => println!("{} filtres actifs", reglages.filters.len()),
        (Err(erreur), _) => eprintln!("{erreur}"),
        (_, Err(erreur)) => eprintln!("{erreur}"),
    }
}
