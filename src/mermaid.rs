use crate::domain::{CliError, DiagramBlock, DiagramErrorDetail, ThemeMode};
use mermaid_svg::Theme;
use std::borrow::Cow;

/// Sélectionne la configuration du thème mermaid-svg.
#[must_use]
pub fn map_theme(theme_mode: ThemeMode) -> Theme {
    match theme_mode {
        ThemeMode::Dark => Theme::dark(),
        ThemeMode::Light => Theme::default_theme(),
        ThemeMode::Neutral => Theme::neutral(),
        ThemeMode::Amber => create_amber_theme(),
        ThemeMode::Phosphor => create_phosphor_theme(),
        ThemeMode::Neon => create_neon_theme(),
        ThemeMode::Mono => create_mono_theme(),
    }
}

fn create_amber_theme() -> Theme {
    Theme {
        fg: Cow::Borrowed("#FFB000"),
        fg_muted: Cow::Borrowed("#CC8800"),
        bg: Cow::Borrowed("#121008"),
        actor_fill: Cow::Borrowed("#2B1D00"),
        actor_stroke: Cow::Borrowed("#FFB000"),
        lifeline: Cow::Borrowed("#805500"),
        arrow_stroke: Cow::Borrowed("#FFB000"),
        note_fill: Cow::Borrowed("#332200"),
        note_stroke: Cow::Borrowed("#FFB000"),
        activation_fill: Cow::Borrowed("#2B1D00"),
        activation_stroke: Cow::Borrowed("#FFB000"),
        frame_label_fill: Cow::Borrowed("#2B1D00"),
        flow_node_fill: Cow::Borrowed("#2B1D00"),
        flow_node_stroke: Cow::Borrowed("#FFB000"),
        flow_edge_stroke: Cow::Borrowed("#FFB000"),
        flow_label_bg: Cow::Borrowed("#121008"),
        flow_cluster_fill: Cow::Borrowed("#1F1500"),
        flow_cluster_stroke: Cow::Borrowed("#CC8800"),
        ..Theme::dark()
    }
}

fn create_phosphor_theme() -> Theme {
    Theme {
        fg: Cow::Borrowed("#33FF33"),
        fg_muted: Cow::Borrowed("#22AA22"),
        bg: Cow::Borrowed("#0A140A"),
        actor_fill: Cow::Borrowed("#0D2B0D"),
        actor_stroke: Cow::Borrowed("#33FF33"),
        lifeline: Cow::Borrowed("#1E551E"),
        arrow_stroke: Cow::Borrowed("#33FF33"),
        note_fill: Cow::Borrowed("#143314"),
        note_stroke: Cow::Borrowed("#33FF33"),
        activation_fill: Cow::Borrowed("#0D2B0D"),
        activation_stroke: Cow::Borrowed("#33FF33"),
        frame_label_fill: Cow::Borrowed("#0D2B0D"),
        flow_node_fill: Cow::Borrowed("#0D2B0D"),
        flow_node_stroke: Cow::Borrowed("#33FF33"),
        flow_edge_stroke: Cow::Borrowed("#33FF33"),
        flow_label_bg: Cow::Borrowed("#0A140A"),
        flow_cluster_fill: Cow::Borrowed("#0B1F0B"),
        flow_cluster_stroke: Cow::Borrowed("#22AA22"),
        ..Theme::dark()
    }
}

fn create_neon_theme() -> Theme {
    Theme {
        fg: Cow::Borrowed("#00F0FF"),
        fg_muted: Cow::Borrowed("#D946EF"),
        bg: Cow::Borrowed("#0D0B18"),
        actor_fill: Cow::Borrowed("#241442"),
        actor_stroke: Cow::Borrowed("#00F0FF"),
        lifeline: Cow::Borrowed("#8B5CF6"),
        arrow_stroke: Cow::Borrowed("#FF007F"),
        note_fill: Cow::Borrowed("#2E1035"),
        note_stroke: Cow::Borrowed("#FF007F"),
        activation_fill: Cow::Borrowed("#241442"),
        activation_stroke: Cow::Borrowed("#00F0FF"),
        frame_label_fill: Cow::Borrowed("#241442"),
        flow_node_fill: Cow::Borrowed("#241442"),
        flow_node_stroke: Cow::Borrowed("#00F0FF"),
        flow_edge_stroke: Cow::Borrowed("#FF007F"),
        flow_label_bg: Cow::Borrowed("#0D0B18"),
        flow_cluster_fill: Cow::Borrowed("#18112C"),
        flow_cluster_stroke: Cow::Borrowed("#D946EF"),
        ..Theme::dark()
    }
}

fn create_mono_theme() -> Theme {
    Theme {
        fg: Cow::Borrowed("#FFFFFF"),
        fg_muted: Cow::Borrowed("#AAAAAA"),
        bg: Cow::Borrowed("#000000"),
        actor_fill: Cow::Borrowed("#000000"),
        actor_stroke: Cow::Borrowed("#FFFFFF"),
        lifeline: Cow::Borrowed("#888888"),
        arrow_stroke: Cow::Borrowed("#FFFFFF"),
        note_fill: Cow::Borrowed("#111111"),
        note_stroke: Cow::Borrowed("#FFFFFF"),
        activation_fill: Cow::Borrowed("#000000"),
        activation_stroke: Cow::Borrowed("#FFFFFF"),
        frame_label_fill: Cow::Borrowed("#000000"),
        flow_node_fill: Cow::Borrowed("#000000"),
        flow_node_stroke: Cow::Borrowed("#FFFFFF"),
        flow_edge_stroke: Cow::Borrowed("#FFFFFF"),
        flow_label_bg: Cow::Borrowed("#000000"),
        flow_cluster_fill: Cow::Borrowed("#000000"),
        flow_cluster_stroke: Cow::Borrowed("#FFFFFF"),
        ..Theme::dark()
    }
}

/// Extrait les informations d'erreur détaillées (message, ligne, catégorie).
#[must_use]
pub fn extract_error_detail(err: &mermaid_svg::RenderError) -> DiagramErrorDetail {
    match err {
        mermaid_svg::RenderError::Parse(parse_err) => match parse_err {
            mermaid_svg::ParseError::Syntax {
                kind,
                message,
                line,
            } => DiagramErrorDetail::new(message.clone(), Some(*line), Some(format!("{kind:?}"))),
            mermaid_svg::ParseError::UnknownDiagramType(diag_type) => DiagramErrorDetail::new(
                format!("type de diagramme inconnu: {diag_type}"),
                Some(1),
                Some("UnknownDiagramType".to_string()),
            ),
            mermaid_svg::ParseError::Empty => DiagramErrorDetail::new(
                "diagramme Mermaid vide".to_string(),
                Some(1),
                Some("Empty".to_string()),
            ),
            _ => DiagramErrorDetail::new(
                format!("{parse_err}"),
                None,
                Some("ParseError".to_string()),
            ),
        },
        _ => DiagramErrorDetail::new(format!("{err}"), None, Some("RenderError".to_string())),
    }
}

/// Moteur abstrait de génération vectorielle SVG pour diagrammes Mermaid.
pub trait DiagramEngine: Send + Sync {
    /// Nom d'identification du moteur (ex. "mermaid-svg", "merman").
    fn name(&self) -> &'static str;

    /// Compile un bloc de diagramme Mermaid en document SVG vectoriel.
    ///
    /// # Errors
    /// Renvoie `DiagramErrorDetail` si la syntaxe Mermaid est invalide ou non supportée.
    fn render_svg(
        &self,
        diagram: &DiagramBlock,
        theme: ThemeMode,
    ) -> Result<String, DiagramErrorDetail>;
}

/// Moteur de rendu SVG natif Rust basé sur `mermaid-svg`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MermaidSvgEngine;

impl DiagramEngine for MermaidSvgEngine {
    fn name(&self) -> &'static str {
        "mermaid-svg"
    }

    fn render_svg(
        &self,
        diagram: &DiagramBlock,
        theme: ThemeMode,
    ) -> Result<String, DiagramErrorDetail> {
        let theme_cfg = map_theme(theme);
        mermaid_svg::render_with(diagram.as_str(), &theme_cfg)
            .map_err(|err| extract_error_detail(&err))
    }
}

static DEFAULT_ENGINE: MermaidSvgEngine = MermaidSvgEngine;

#[must_use]
pub fn default_engine() -> &'static dyn DiagramEngine {
    &DEFAULT_ENGINE
}

/// Seuil de largeur (en colonnes) en dessous duquel un étalement horizontal (LR/RL)
/// est automatiquement adapté en cascade verticale (TD).
pub const AUTOFOLD_WIDTH_THRESHOLD: u16 = 120;

/// Adapte dynamiquement la disposition d'un diagramme Mermaid à la largeur du terminal.
///
/// Si la largeur disponible est inférieure à `AUTOFOLD_WIDTH_THRESHOLD` et que le diagramme
/// est orienté horizontalement (`graph LR`, `flowchart LR`, `graph RL`, `flowchart RL`),
/// l'orientation est automatiquement convertie en `TD` (Top-Down).
#[must_use]
pub fn adapt_direction_for_viewport(source: &str, target_cols: u16) -> Cow<'_, str> {
    if target_cols >= AUTOFOLD_WIDTH_THRESHOLD {
        return Cow::Borrowed(source);
    }

    if let Some((range, _orig_dir)) = find_main_direction_token(source) {
        let mut adapted = String::with_capacity(source.len());
        adapted.push_str(&source[..range.start]);
        adapted.push_str("TD");
        adapted.push_str(&source[range.end..]);
        Cow::Owned(adapted)
    } else {
        Cow::Borrowed(source)
    }
}

fn find_main_direction_token(source: &str) -> Option<(std::ops::Range<usize>, &'static str)> {
    let mut cursor = 0;
    while cursor < source.len() {
        let remainder = &source[cursor..];
        let line = match remainder.split_once('\n') {
            Some((l, _)) => l.strip_suffix('\r').unwrap_or(l),
            None => remainder,
        };

        let trimmed = line.trim_start();
        if !trimmed.is_empty() && !trimmed.starts_with("%%") {
            let indent = line.len() - trimmed.len();
            return inspect_header_line(trimmed, cursor + indent);
        }

        cursor += line.len();
        if cursor < source.len() && source[cursor..].starts_with("\r\n") {
            cursor += 2;
        } else if cursor < source.len()
            && (source[cursor..].starts_with('\n') || source[cursor..].starts_with('\r'))
        {
            cursor += 1;
        }
    }
    None
}

fn inspect_header_line(
    trimmed_line: &str,
    base_offset: usize,
) -> Option<(std::ops::Range<usize>, &'static str)> {
    let (keyword_len, rest) = if let Some(r) = trimmed_line.strip_prefix("graph") {
        (5, r)
    } else {
        let r = trimmed_line.strip_prefix("flowchart")?;
        (9, r)
    };

    let rest_trimmed = rest.trim_start();
    let ws_len = rest.len() - rest_trimmed.len();
    if ws_len == 0 {
        return None;
    }

    let token_offset = base_offset + keyword_len + ws_len;
    check_direction_token(rest_trimmed, token_offset)
}

fn check_direction_token(
    rest: &str,
    token_offset: usize,
) -> Option<(std::ops::Range<usize>, &'static str)> {
    if rest.starts_with("LR") && is_direction_boundary(&rest[2..]) {
        return Some((token_offset..token_offset + 2, "LR"));
    }
    if rest.starts_with("RL") && is_direction_boundary(&rest[2..]) {
        return Some((token_offset..token_offset + 2, "RL"));
    }
    None
}

fn is_direction_boundary(text: &str) -> bool {
    match text.chars().next() {
        None => true,
        Some(c) => c.is_whitespace() || c == ';' || c == '%' || c == '\n' || c == '\r',
    }
}

/// Compile un bloc de diagramme Mermaid en document SVG vectoriel avec diagnostic détaillé.
///
/// # Errors
/// Renvoie `DiagramErrorDetail` si la syntaxe Mermaid est invalide.
pub fn render_to_svg_detailed(
    diagram: &DiagramBlock,
    theme_mode: ThemeMode,
) -> Result<String, DiagramErrorDetail> {
    default_engine().render_svg(diagram, theme_mode)
}

/// Compile un bloc de diagramme Mermaid en document SVG vectoriel.
///
/// # Errors
/// Renvoie `CliError::MermaidSyntax` si la syntaxe Mermaid est invalide.
pub fn render_to_svg(diagram: &DiagramBlock, theme_mode: ThemeMode) -> Result<String, CliError> {
    render_to_svg_detailed(diagram, theme_mode).map_err(|detail| {
        if let Some(line) = detail.line {
            CliError::MermaidSyntax(format!("ligne {line}: {}", detail.message))
        } else {
            CliError::MermaidSyntax(detail.message)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_valid_flowchart() {
        let block = DiagramBlock::new("graph TD\n  A[Start] --> B[End]".to_string());
        let svg = render_to_svg(&block, ThemeMode::Dark);
        assert!(svg.is_ok());
        let svg_str = svg.unwrap_or_default();
        assert!(svg_str.contains("<svg"));
    }

    #[test]
    fn test_render_invalid_mermaid() {
        let block = DiagramBlock::new("invalid diagram syntax $$$$".to_string());
        let svg = render_to_svg(&block, ThemeMode::Dark);
        assert!(svg.is_err());
    }

    #[test]
    fn test_render_invalid_mermaid_detailed_reports_line_and_kind() {
        let block = DiagramBlock::new("graph TD\n  invalid syntax line 2 $$$".to_string());
        let result = render_to_svg_detailed(&block, ThemeMode::Dark);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.line, Some(2));
            assert!(err.kind.is_some());
        }
    }

    #[test]
    fn test_render_unknown_diagram_type_reports_line_one() {
        let block = DiagramBlock::new("unknown_type_diagram\n  A --> B".to_string());
        let result = render_to_svg_detailed(&block, ThemeMode::Dark);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.line, Some(1));
            assert_eq!(err.kind, Some("UnknownDiagramType".to_string()));
        }
    }

    #[test]
    fn test_render_retro_themes_flowchart() {
        let block = DiagramBlock::new("graph LR\n  A --> B".to_string());
        let themes = [
            ThemeMode::Amber,
            ThemeMode::Phosphor,
            ThemeMode::Neon,
            ThemeMode::Mono,
        ];
        for theme in themes {
            let svg = render_to_svg(&block, theme);
            assert!(svg.is_ok());
            let svg_str = svg.unwrap_or_default();
            assert!(svg_str.contains("<svg"));
        }
    }

    #[test]
    fn test_diagram_engine_default_name() {
        let engine = default_engine();
        assert_eq!(engine.name(), "mermaid-svg");
    }

    #[test]
    fn test_adapt_direction_narrow_converts_lr_to_td() {
        let input = "graph LR\n  A --> B --> C";
        let adapted = adapt_direction_for_viewport(input, 80);
        assert_eq!(adapted, "graph TD\n  A --> B --> C");
    }

    #[test]
    fn test_adapt_direction_narrow_converts_flowchart_rl_to_td() {
        let input = "flowchart RL; A-->B";
        let adapted = adapt_direction_for_viewport(input, 60);
        assert_eq!(adapted, "flowchart TD; A-->B");
    }

    #[test]
    fn test_adapt_direction_wide_preserves_lr() {
        let input = "graph LR\n  A --> B --> C";
        let adapted = adapt_direction_for_viewport(input, 140);
        assert_eq!(adapted, input);
        assert!(matches!(adapted, Cow::Borrowed(_)));
    }

    #[test]
    fn test_adapt_direction_preserves_td() {
        let input = "graph TD\n  A --> B";
        let adapted = adapt_direction_for_viewport(input, 50);
        assert_eq!(adapted, input);
    }

    #[test]
    fn test_adapt_direction_preserves_labels_containing_lr() {
        let input = "graph LR\n  A[\"Cache LRU Engine\"] --> B";
        let adapted = adapt_direction_for_viewport(input, 80);
        assert_eq!(adapted, "graph TD\n  A[\"Cache LRU Engine\"] --> B");
    }
}
