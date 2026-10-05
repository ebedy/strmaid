use crate::domain::{CliError, GraphicsProtocol, OutputFormat, ThemeMode};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Strmaid: Rendu fluide de flux Markdown et diagrammes Mermaid en terminal.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
pub struct CliArgs {
    /// Sous-commande optionnelle (ex. doctor).
    #[command(subcommand)]
    pub command: Option<Subcommands>,

    /// Fichier Markdown à lire (si omis, lit depuis stdin).
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Forcer le mode pager interactif (TUI Ratatui).
    #[arg(short = 'p', long = "pager", conflicts_with = "no_pager")]
    pub force_pager: bool,

    /// Désactiver le pager et forcer le mode filtre Unix composable (stdin -> stdout).
    #[arg(long = "no-pager", conflicts_with = "force_pager")]
    pub no_pager: bool,

    /// Protocole graphique à utiliser (kitty, halfblocks, asciibox, raw). Détecté automatiquement par défaut.
    #[arg(short = 'g', long = "graphics", value_name = "PROTOCOL", value_enum)]
    pub graphics: Option<GraphicsProtocol>,

    /// Thème visuel Mermaid (dark, light, neutral).
    #[arg(
        short = 't',
        long = "theme",
        value_name = "THEME",
        value_enum,
        default_value_t = ThemeMode::Dark
    )]
    pub theme: ThemeMode,

    /// Format de sortie des flux analysés (human, json, ndjson).
    #[arg(
        long = "format",
        value_name = "FORMAT",
        value_enum,
        default_value_t = OutputFormat::Human
    )]
    pub format: OutputFormat,

    /// Mode visualiseur pour éditeurs : traite l'entrée entière comme un bloc Mermaid unique.
    #[arg(short = 'b', long = "block-only")]
    pub block_only: bool,

    /// Largeur personnalisée en colonnes (surcharge la détection automatique du terminal).
    #[arg(short = 'w', long = "width", value_name = "COLS")]
    pub width: Option<u16>,

    /// Désactiver l'auto-orientation automatique (LR/RL -> TD) sur les terminaux étroits.
    #[arg(long = "no-auto-orient")]
    pub no_auto_orient: bool,
}

/// Sous-commandes disponibles pour `strmaid`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Subcommands {
    /// Diagnostique les capacités matérielles et logicielles du terminal hôte.
    Doctor {
        /// Format de sortie du diagnostic (human, json, ndjson).
        #[arg(
            long = "format",
            value_name = "FORMAT",
            value_enum,
            default_value_t = OutputFormat::Human
        )]
        format: OutputFormat,
    },
    /// Démarre le serveur Model Context Protocol (MCP) sur standard I/O (JSON-RPC 2.0).
    Mcp,
    /// Exécute une commande dans un pseudo-terminal (PTY) interactif en interceptant les diagrammes Mermaid.
    Run {
        /// Commande et arguments à exécuter dans le PTY.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}

impl CliArgs {
    /// Valide la cohérence des arguments passés en ligne de commande.
    ///
    /// # Errors
    /// Renvoie une erreur en cas de conflit d'options ou de paramètre invalide.
    pub fn validate(&self) -> Result<(), CliError> {
        if self.command.is_some() {
            return Ok(());
        }
        if self.force_pager && self.format != OutputFormat::Human {
            return Err(CliError::TerminalInit(
                "l'option --pager est incompatible avec les formats machine-readable (--format json / ndjson)".to_string(),
            ));
        }
        if self.block_only && self.force_pager {
            return Err(CliError::TerminalInit(
                "l'option --block-only est incompatible avec le mode pager interactif (--pager)"
                    .to_string(),
            ));
        }
        if self.width == Some(0) {
            return Err(CliError::TerminalInit(
                "la largeur --width doit être strictement supérieure à 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_defaults() {
        let args = CliArgs::try_parse_from(["strmaid"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.file, None);
        assert!(!parsed.force_pager);
        assert!(!parsed.no_pager);
        assert_eq!(parsed.theme, ThemeMode::Dark);
        assert_eq!(parsed.graphics, None);
    }

    #[test]
    fn test_cli_parse_valid_theme_and_graphics() {
        let args = CliArgs::try_parse_from([
            "strmaid",
            "--theme",
            "light",
            "--graphics",
            "halfblocks",
            "input.md",
        ]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.theme, ThemeMode::Light);
        assert_eq!(parsed.graphics, Some(GraphicsProtocol::HalfBlocks));
        assert_eq!(parsed.file, Some(PathBuf::from("input.md")));
    }

    #[test]
    fn test_cli_parse_graphics_alias() {
        let args = CliArgs::try_parse_from(["strmaid", "-g", "halfblock"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.graphics, Some(GraphicsProtocol::HalfBlocks));
    }

    #[test]
    fn test_cli_parse_graphics_asciibox() {
        let args = CliArgs::try_parse_from(["strmaid", "-g", "asciibox"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.graphics, Some(GraphicsProtocol::AsciiBox));

        let args_alias = CliArgs::try_parse_from(["strmaid", "--graphics", "ascii"]);
        assert!(args_alias.is_ok());
        let parsed_alias = args_alias.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed_alias.graphics, Some(GraphicsProtocol::AsciiBox));
    }

    #[test]
    fn test_cli_parse_no_auto_orient() {
        let default_args = CliArgs::try_parse_from(["strmaid"]);
        assert!(default_args.is_ok());
        let parsed_default = default_args.unwrap_or_else(|_| unreachable!());
        assert!(!parsed_default.no_auto_orient);

        let explicit_args = CliArgs::try_parse_from(["strmaid", "--no-auto-orient"]);
        assert!(explicit_args.is_ok());
        let parsed_explicit = explicit_args.unwrap_or_else(|_| unreachable!());
        assert!(parsed_explicit.no_auto_orient);
    }

    #[test]
    fn test_cli_rejects_invalid_theme() {
        let args = CliArgs::try_parse_from(["strmaid", "--theme", "invalid-theme"]);
        assert!(args.is_err());
    }

    #[test]
    fn test_cli_rejects_invalid_graphics() {
        let args = CliArgs::try_parse_from(["strmaid", "--graphics", "sixel-unsupported"]);
        assert!(args.is_err());
    }

    #[test]
    fn test_cli_rejects_conflicting_pager_flags() {
        let args = CliArgs::try_parse_from(["strmaid", "--pager", "--no-pager"]);
        assert!(args.is_err());
    }

    #[test]
    fn test_cli_parse_format_options() {
        let args_json = CliArgs::try_parse_from(["strmaid", "--format", "json"]);
        assert!(args_json.is_ok());
        let parsed_json = args_json.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed_json.format, OutputFormat::Json);

        let args_ndjson = CliArgs::try_parse_from(["strmaid", "--format", "ndjson"]);
        assert!(args_ndjson.is_ok());
        let parsed_ndjson = args_ndjson.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed_ndjson.format, OutputFormat::Ndjson);
    }

    #[test]
    fn test_cli_rejects_invalid_format() {
        let args = CliArgs::try_parse_from(["strmaid", "--format", "xml"]);
        assert!(args.is_err());
    }

    #[test]
    fn test_cli_validate_rejects_pager_with_json() {
        let args = CliArgs::try_parse_from(["strmaid", "--pager", "--format", "json"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert!(parsed.validate().is_err());
    }

    #[test]
    fn test_cli_validate_accepts_valid_combinations() {
        let args = CliArgs::try_parse_from(["strmaid", "--format", "json"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert!(parsed.validate().is_ok());

        let args_pager = CliArgs::try_parse_from(["strmaid", "--pager"]);
        assert!(args_pager.is_ok());
        let parsed_pager = args_pager.unwrap_or_else(|_| unreachable!());
        assert!(parsed_pager.validate().is_ok());
    }

    #[test]
    fn test_cli_parse_block_only_and_width() {
        let args = CliArgs::try_parse_from(["strmaid", "-b", "-w", "60", "input.mmd"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert!(parsed.block_only);
        assert_eq!(parsed.width, Some(60));
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_cli_validate_rejects_block_only_with_pager() {
        let args = CliArgs::try_parse_from(["strmaid", "--block-only", "--pager"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert!(parsed.validate().is_err());
    }

    #[test]
    fn test_cli_validate_rejects_zero_width() {
        let args = CliArgs::try_parse_from(["strmaid", "--width", "0"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert!(parsed.validate().is_err());
    }

    #[test]
    fn test_cli_parse_retro_themes() {
        for (theme_str, expected) in [
            ("amber", ThemeMode::Amber),
            ("phosphor", ThemeMode::Phosphor),
            ("neon", ThemeMode::Neon),
            ("mono", ThemeMode::Mono),
        ] {
            let args = CliArgs::try_parse_from(["strmaid", "--theme", theme_str]);
            assert!(args.is_ok());
            let parsed = args.unwrap_or_else(|_| unreachable!());
            assert_eq!(parsed.theme, expected);
        }
    }

    #[test]
    fn test_cli_parse_subcommand_doctor() {
        let args = CliArgs::try_parse_from(["strmaid", "doctor"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(
            parsed.command,
            Some(Subcommands::Doctor {
                format: OutputFormat::Human
            })
        );
        assert!(parsed.validate().is_ok());

        let args_json = CliArgs::try_parse_from(["strmaid", "doctor", "--format", "json"]);
        assert!(args_json.is_ok());
        let parsed_json = args_json.unwrap_or_else(|_| unreachable!());
        assert_eq!(
            parsed_json.command,
            Some(Subcommands::Doctor {
                format: OutputFormat::Json
            })
        );
        assert!(parsed_json.validate().is_ok());
    }

    #[test]
    fn test_cli_parse_subcommand_mcp() {
        let args = CliArgs::try_parse_from(["strmaid", "mcp"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.command, Some(Subcommands::Mcp));
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_cli_parse_subcommand_run() {
        let args = CliArgs::try_parse_from(["strmaid", "run", "--", "echo", "hello"]);
        assert!(args.is_ok());
        let parsed = args.unwrap_or_else(|_| unreachable!());
        assert_eq!(
            parsed.command,
            Some(Subcommands::Run {
                command: vec!["echo".to_string(), "hello".to_string()]
            })
        );
        assert!(parsed.validate().is_ok());
    }
}
