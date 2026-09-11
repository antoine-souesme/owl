//! Notifications du système, hors du terminal.
//!
//! Sur macOS, la notification est confiée à `osascript`, présent d'origine :
//! aucune bibliothèque supplémentaire à embarquer. L'échec reste silencieux —
//! une notification perdue ne doit pas abîmer l'écran ni arrêter `owl`.

use std::process::Command;

/// Envoie la notification. Ne rend rien : un refus du système, une machine
/// sans `osascript`, un utilisateur qui a coupé les notifications de son
/// terminal ne sont pas des erreurs de `owl`.
pub fn send(title: &str, body: &str) {
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(script(title, body))
        .output();
}

/// Ligne d'AppleScript affichant la notification.
///
/// Le titre et le corps viennent de GitHub : ils peuvent porter n'importe
/// quel caractère, guillemet compris. Les échapper est ce qui empêche un
/// titre de pull request de se faire passer pour du code.
fn script(title: &str, body: &str) -> String {
    format!(
        "display notification \"{}\" with title \"{}\"",
        quoted(body),
        quoted(title)
    )
}

/// Contenu d'une chaîne d'AppleScript : la barre oblique inverse d'abord,
/// sans quoi elle échapperait les guillemets ajoutés juste après.
fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_carries_the_title_and_the_body() {
        assert_eq!(
            script("Ready to merge", "moi/owl #42 · Un titre"),
            "display notification \"moi/owl #42 · Un titre\" with title \"Ready to merge\""
        );
    }

    #[test]
    fn a_quote_in_the_text_cannot_break_out_of_the_script() {
        assert_eq!(
            script("t", "un \"titre\" cité"),
            "display notification \"un \\\"titre\\\" cité\" with title \"t\""
        );
    }

    #[test]
    fn a_backslash_in_the_text_is_escaped_too() {
        assert_eq!(
            script("t", "a\\b"),
            "display notification \"a\\\\b\" with title \"t\""
        );
    }
}
