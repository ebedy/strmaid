pub mod asciibox;
pub mod halfblock;
pub mod iterm2;
pub mod kitty;

use crate::domain::GraphicsProtocol;
use std::collections::BTreeMap;
use std::env;

/// Variables d'environnement consultées par la détection du protocole graphique.
pub const DETECTION_ENV_VARS: [&str; 7] = [
    "KITTY_WINDOW_ID",
    "TERM_PROGRAM",
    "TERM",
    "COLORTERM",
    "NO_COLOR",
    "TMUX",
    "STY",
];

/// Instantané des variables d'environnement du terminal, injecté pour rendre la
/// détection pure et testable sans muter l'environnement du processus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalEnvironment {
    variables: BTreeMap<String, String>,
}

impl TerminalEnvironment {
    /// Lit les variables de `DETECTION_ENV_VARS` dans l'environnement du processus.
    #[must_use]
    pub fn from_process() -> Self {
        Self::from_pairs(
            DETECTION_ENV_VARS
                .iter()
                .filter_map(|&name| env::var(name).ok().map(|value| (name, value))),
        )
    }

    /// Construit un instantané à partir de paires (nom, valeur).
    pub fn from_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            variables: pairs
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    fn is_set(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    fn lowercase(&self, name: &str) -> Option<String> {
        self.variables
            .get(name)
            .map(|value| value.to_ascii_lowercase())
    }

    fn contains_any(&self, name: &str, needles: &[&str]) -> bool {
        self.lowercase(name)
            .is_some_and(|value| needles.iter().any(|needle| value.contains(needle)))
    }

    /// Session tmux (`TMUX`) ou GNU screen (`STY`) : les séquences graphiques Kitty
    /// et iTerm2 y sont filtrées sans passthrough explicite.
    #[must_use]
    pub fn is_multiplexed(&self) -> bool {
        self.is_set("TMUX") || self.is_set("STY")
    }
}

/// Détecte le protocole graphique supporté par l'environnement terminal du processus.
#[must_use]
pub fn detect_protocol(override_opt: Option<GraphicsProtocol>) -> GraphicsProtocol {
    detect_protocol_in(override_opt, &TerminalEnvironment::from_process())
}

/// Détecte le protocole graphique pour un environnement donné. Un choix explicite est
/// toujours respecté ; sous multiplexeur, Kitty et iTerm2 ne sont jamais sélectionnés
/// automatiquement.
#[must_use]
pub fn detect_protocol_in(
    override_opt: Option<GraphicsProtocol>,
    environment: &TerminalEnvironment,
) -> GraphicsProtocol {
    if let Some(proto) = override_opt {
        return proto;
    }
    if !environment.is_multiplexed() && is_kitty_supported(environment) {
        return GraphicsProtocol::Kitty;
    }
    if !environment.is_multiplexed() && is_iterm2_supported(environment) {
        return GraphicsProtocol::Iterm2;
    }
    if is_truecolor_supported(environment) {
        GraphicsProtocol::HalfBlocks
    } else {
        GraphicsProtocol::AsciiBox
    }
}

/// Prise en charge du mode ANSI `TrueColor` 24-bit.
fn is_truecolor_supported(environment: &TerminalEnvironment) -> bool {
    if environment.is_set("NO_COLOR") {
        return false;
    }
    if environment.contains_any("COLORTERM", &["truecolor", "24bit"]) {
        return true;
    }
    !matches!(
        environment.lowercase("TERM").as_deref(),
        Some("dumb" | "linux")
    )
}

/// Prise en charge du Kitty Graphics Protocol.
fn is_kitty_supported(environment: &TerminalEnvironment) -> bool {
    environment.is_set("KITTY_WINDOW_ID")
        || environment.contains_any("TERM_PROGRAM", &["warp", "wezterm", "ghostty", "kitty"])
        || environment.contains_any("TERM", &["kitty", "ghostty", "wezterm"])
}

/// Prise en charge du protocole iTerm2 Inline Images.
fn is_iterm2_supported(environment: &TerminalEnvironment) -> bool {
    environment.contains_any("TERM_PROGRAM", &["iterm"])
        || environment.contains_any("TERM", &["iterm"])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(pairs: &[(&str, &str)], override_opt: Option<GraphicsProtocol>) -> GraphicsProtocol {
        detect_protocol_in(
            override_opt,
            &TerminalEnvironment::from_pairs(pairs.iter().copied()),
        )
    }

    #[test]
    fn test_detect_protocol_in_native_terminals() {
        assert_eq!(
            detect(&[("KITTY_WINDOW_ID", "1")], None),
            GraphicsProtocol::Kitty
        );
        assert_eq!(
            detect(&[("TERM_PROGRAM", "WezTerm")], None),
            GraphicsProtocol::Kitty
        );
        assert_eq!(
            detect(&[("TERM_PROGRAM", "iTerm.app")], None),
            GraphicsProtocol::Iterm2
        );
        assert_eq!(detect(&[], None), GraphicsProtocol::HalfBlocks);
        assert_eq!(
            detect(&[("TERM", "dumb")], None),
            GraphicsProtocol::AsciiBox
        );
    }

    #[test]
    fn test_detect_protocol_in_multiplexer_falls_back_from_image_protocols() {
        let tmux_in_kitty = [
            ("TMUX", "/tmp/tmux-1000/default,1,0"),
            ("KITTY_WINDOW_ID", "1"),
            ("COLORTERM", "truecolor"),
        ];
        assert_eq!(detect(&tmux_in_kitty, None), GraphicsProtocol::HalfBlocks);

        let screen_in_iterm_without_color = [
            ("STY", "1234.pts-0"),
            ("TERM_PROGRAM", "iTerm.app"),
            ("NO_COLOR", "1"),
        ];
        assert_eq!(
            detect(&screen_in_iterm_without_color, None),
            GraphicsProtocol::AsciiBox
        );
    }

    #[test]
    fn test_detect_protocol_in_keeps_explicit_choice_under_multiplexer() {
        let tmux = [("TMUX", "/tmp/tmux-1000/default,1,0")];
        assert_eq!(
            detect(&tmux, Some(GraphicsProtocol::Kitty)),
            GraphicsProtocol::Kitty
        );
    }

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
