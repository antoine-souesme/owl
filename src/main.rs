mod token;

fn main() {
    match token::resolve() {
        Ok(_) => println!("jeton trouvé"),
        Err(erreur) => eprintln!("{erreur}"),
    }
}
