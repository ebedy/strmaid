pub mod asciibox;
pub mod halfblock;
pub mod iterm2;
pub mod kitty;

use crate::domain::GraphicsProtocol;
use std::env;

/// Détecte le protocole graphique supporté par l'environnement terminal.
#[must_use]
pub fn detect_protocol(override_opt: Option<GraphicsProtocol>) -> GraphicsProtocol {
    if let Some(proto) = override_opt {
        return proto;
    }

    if is_kitty_supported_in_env() {
        GraphicsProtocol::Kitty
    } else if is_iterm2_supported_in_env() {
        GraphicsProtocol::Iterm2
    } else if is_truecolor_supported_in_env() {
        GraphicsProtocol::HalfBlocks
    } else {
        GraphicsProtocol::AsciiBox
    }
}

/// Vérifie si l'environnement d'exécution prend en charge le mode ANSI `TrueColor` 24-bit.
fn is_truecolor_supported_in_env() -> bool {
    if env::var("NO_COLOR").is_ok() {
        return false;
    }

    if let Ok(colorterm) = env::var("COLORTERM") {
        let ct = colorterm.to_ascii_lowercase();
        if ct.contains("truecolor") || ct.contains("24bit") {
            return true;
        }
    }

    if let Ok(term) = env::var("TERM") {
        let term_lower = term.to_ascii_lowercase();
        if term_lower == "dumb" || term_lower == "linux" {
            return false;
        }
        if term_lower.contains("direct")
            || term_lower.contains("24bit")
            || term_lower.contains("256color")
        {
            return true;
        }
    }

    true
}

/// Vérifie si l'environnement d'exécution prend en charge le Kitty Graphics Protocol.
fn is_kitty_supported_in_env() -> bool {
    if env::var("KITTY_WINDOW_ID").is_ok() {
        return true;
    }

    if let Ok(prog) = env::var("TERM_PROGRAM") {
        let prog_lower = prog.to_ascii_lowercase();
        if prog_lower.contains("warp")
            || prog_lower.contains("wezterm")
            || prog_lower.contains("ghostty")
            || prog_lower.contains("kitty")
        {
            return true;
        }
    }

    if let Ok(term) = env::var("TERM") {
        let term_lower = term.to_ascii_lowercase();
        if term_lower.contains("kitty")
            || term_lower.contains("ghostty")
            || term_lower.contains("wezterm")
        {
            return true;
        }
    }

    false
}

/// Vérifie si l'environnement d'exécution prend en charge le protocole iTerm2 Inline Images.
fn is_iterm2_supported_in_env() -> bool {
    let check_var =
        |var: &str| env::var(var).is_ok_and(|v| v.to_ascii_lowercase().contains("iterm"));
    check_var("TERM_PROGRAM") || check_var("TERM")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_protocol_with_override() {
        let proto = detect_protocol(Some(GraphicsProtocol::Raw));
        assert_eq!(proto, GraphicsProtocol::Raw);

        let ascii_proto = detect_protocol(Some(GraphicsProtocol::AsciiBox));
        assert_eq!(ascii_proto, GraphicsProtocol::AsciiBox);

        let iterm_proto = detect_protocol(Some(GraphicsProtocol::Iterm2));
        assert_eq!(iterm_proto, GraphicsProtocol::Iterm2);
    }
}
