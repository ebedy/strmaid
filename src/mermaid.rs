use crate::domain::{CliError, DiagramBlock, DiagramEngineType, DiagramErrorDetail, ThemeMode};
use clap::ValueEnum;
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

#[cfg(feature = "merman")]
static MERMAN_ENGINE: MermanEngine = MermanEngine;

#[must_use]
pub fn default_engine() -> &'static dyn DiagramEngine {
    &DEFAULT_ENGINE
}

/// Sélectionne un moteur de rendu par son nom.
///
/// # Errors
/// Renvoie `CliError::InvalidArgument` si le moteur demandé est inconnu ou non compilé.
pub fn engine_by_name(name: &str) -> Result<&'static dyn DiagramEngine, CliError> {
    match name {
        "mermaid-svg" => Ok(&DEFAULT_ENGINE),
        #[cfg(feature = "merman")]
        "merman" => Ok(&MERMAN_ENGINE),
        #[cfg(not(feature = "merman"))]
        "merman" => Err(CliError::CommandLine(
            "Le moteur 'merman' n'est pas activé dans ce binaire (recompilez avec --features merman)".to_string(),
        )),
        other => Err(CliError::CommandLine(format!(
            "Moteur Mermaid inconnu '{other}'. Moteurs disponibles: 'mermaid-svg'{}",
            if cfg!(feature = "merman") { ", 'merman'" } else { "" }
        ))),
    }
}

/// Résout un nom de moteur en `DiagramEngineType`, en vérifiant qu'il est disponible
/// dans ce binaire (mêmes erreurs que [`engine_by_name`]).
///
/// # Errors
/// Renvoie `CliError::CommandLine` si le moteur est inconnu ou non compilé.
pub fn engine_type_by_name(name: &str) -> Result<DiagramEngineType, CliError> {
    engine_by_name(name)?;
    DiagramEngineType::value_variants()
        .iter()
        .copied()
        .find(|engine_type| engine_type.as_str() == name)
        .ok_or_else(|| CliError::CommandLine(format!("Moteur Mermaid inconnu '{name}'")))
}

/// Sélectionne un moteur de rendu par son type d'énumération.
///
/// # Errors
/// Renvoie `CliError::CommandLine` si le moteur demandé n'est pas activé à la compilation.
pub fn get_engine(engine_type: DiagramEngineType) -> Result<&'static dyn DiagramEngine, CliError> {
    engine_by_name(engine_type.as_str())
}

#[cfg(feature = "merman")]
/// Moteur de rendu SVG basé sur `merman` (standard Zed / layout ELK).
#[derive(Debug, Default, Clone, Copy)]
pub struct MermanEngine;

#[cfg(feature = "merman")]
impl DiagramEngine for MermanEngine {
    fn name(&self) -> &'static str {
        "merman"
    }

    fn render_svg(
        &self,
        diagram: &DiagramBlock,
        theme: ThemeMode,
    ) -> Result<String, DiagramErrorDetail> {
        let themed_source = with_merman_theme(diagram.as_str(), theme);
        let output = merman::Renderer::new()
            .render(merman::RenderRequest::svg(
                &themed_source,
                merman::OperationControl::new(),
                merman::SvgRequest::default(),
            ))
            .map_err(|err| {
                DiagramErrorDetail::new(format!("{err}"), None, Some("MermanError".to_string()))
            })?;

        match output {
            merman::RenderOutput::Svg(Some(svg_artifact)) => Ok(svg_artifact.svg().to_string()),
            merman::RenderOutput::Svg(None) => Err(DiagramErrorDetail::new(
                "Merman n'a produit aucun flux SVG".to_string(),
                None,
                Some("EmptyOutput".to_string()),
            )),
            _ => Err(DiagramErrorDetail::new(
                "Merman a produit un format de sortie inattendu".to_string(),
                None,
                Some("UnexpectedOutput".to_string()),
            )),
        }
    }
}

/// Thème Mermaid natif le plus proche de `ThemeMode`, aligné sur `map_theme` : les
/// palettes rétro et `mono` dérivent du thème sombre.
#[cfg(feature = "merman")]
const fn merman_theme_name(theme: ThemeMode) -> &'static str {
    match theme {
        ThemeMode::Light => "default",
        ThemeMode::Neutral => "neutral",
        ThemeMode::Dark
        | ThemeMode::Amber
        | ThemeMode::Phosphor
        | ThemeMode::Neon
        | ThemeMode::Mono => "dark",
    }
}

/// Injecte une directive `init` de thème, `merman` n'exposant pas d'option de thème
/// typée. Placée après un éventuel front matter YAML (qui doit rester en tête) et
/// avant les directives de l'utilisateur, qui gardent ainsi la priorité.
#[cfg(feature = "merman")]
fn with_merman_theme(source: &str, theme: ThemeMode) -> String {
    let directive = format!(
        "%%{{init: {{\"theme\": \"{}\"}}}}%%",
        merman_theme_name(theme)
    );
    let (frontmatter, body) = split_frontmatter(source);
    format!("{frontmatter}{directive}\n{body}")
}

/// Sépare un front matter YAML de tête (`---` … `---`) du reste de la source.
#[cfg(feature = "merman")]
fn split_frontmatter(source: &str) -> (&str, &str) {
    let Some(after_opening) = source.strip_prefix("---\n") else {
        return ("", source);
    };
    after_opening.find("\n---\n").map_or(("", source), |end| {
        source.split_at("---\n".len() + end + "\n---\n".len())
    })
}

/// Caractère Unicode de substitution sécurisé pour l'esperluette en libellé (U+FE60: Small Ampersand).
/// Rendu visuellement identique à `&` tout en neutralisant l'opérateur de chaînage multiple Mermaid.
pub const SAFE_AMPERSAND: char = '﹠';

#[derive(Debug, Default)]
struct LabelSanitizer {
    in_comment: bool,
    quote: Option<char>,
    bracket_depth: usize,
    paren_depth: usize,
    brace_depth: usize,
    in_pipe: bool,
    escape_next: bool,
}

impl LabelSanitizer {
    const fn is_in_label(&self) -> bool {
        self.quote.is_some()
            || self.bracket_depth > 0
            || self.paren_depth > 0
            || self.brace_depth > 0
            || self.in_pipe
    }

    fn process_comment(&mut self, ch: char) {
        if ch == '\n' {
            self.in_comment = false;
        }
    }

    fn update_quotes(&mut self, ch: char) {
        if self.escape_next {
            self.escape_next = false;
            return;
        }
        if ch == '\\' {
            self.escape_next = true;
            return;
        }
        if let Some(q) = self.quote {
            if ch == q {
                self.quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            self.quote = Some(ch);
        }
    }

    fn update_delimiters(&mut self, ch: char) {
        if self.quote.is_some() {
            return;
        }
        match ch {
            '[' => self.bracket_depth = self.bracket_depth.saturating_add(1),
            ']' => self.bracket_depth = self.bracket_depth.saturating_sub(1),
            '(' => self.paren_depth = self.paren_depth.saturating_add(1),
            ')' => self.paren_depth = self.paren_depth.saturating_sub(1),
            '{' => self.brace_depth = self.brace_depth.saturating_add(1),
            '}' => self.brace_depth = self.brace_depth.saturating_sub(1),
            '|' => self.in_pipe = !self.in_pipe,
            _ => {}
        }
    }
}

fn check_ampersand_action(remainder: &str) -> Option<(char, usize)> {
    if remainder.starts_with("&amp;") {
        return Some((SAFE_AMPERSAND, 5));
    }
    if remainder.starts_with("&#38;") {
        return Some((SAFE_AMPERSAND, 5));
    }
    if remainder.starts_with("&lt;")
        || remainder.starts_with("&gt;")
        || remainder.starts_with("&quot;")
        || remainder.starts_with("&apos;")
    {
        return None;
    }
    Some((SAFE_AMPERSAND, 1))
}

/// Assainit les libellés de diagrammes Mermaid pour neutraliser les défaillances de parsing.
///
/// Remplace les esperluettes (`&`) situées à l'intérieur de chaînes délimitées par des guillemets
/// ou des balises de nœuds par le caractère Unicode sécurisé `﹠` (U+FE60), préservant ainsi les liaisons
/// multiples légitimes hors guillemets (ex. `A & B --> C & D`).
#[must_use]
pub fn sanitize_mermaid_labels(source: &str) -> Cow<'_, str> {
    if !source.contains('&') {
        return Cow::Borrowed(source);
    }

    let mut result = String::with_capacity(source.len());
    let mut sanitizer = LabelSanitizer::default();
    let mut cursor = 0;

    while cursor < source.len() {
        let remainder = &source[cursor..];
        let Some(ch) = remainder.chars().next() else {
            break;
        };

        if sanitizer.in_comment {
            result.push(ch);
            sanitizer.process_comment(ch);
            cursor += ch.len_utf8();
            continue;
        }

        if remainder.starts_with("%%") && !sanitizer.is_in_label() {
            sanitizer.in_comment = true;
            result.push_str("%%");
            cursor += 2;
            continue;
        }

        sanitizer.update_quotes(ch);
        sanitizer.update_delimiters(ch);

        if ch == '&'
            && sanitizer.is_in_label()
            && let Some((replacement, advance)) = check_ampersand_action(remainder)
        {
            result.push(replacement);
            cursor += advance;
            continue;
        }

        result.push(ch);
        cursor += ch.len_utf8();
    }

    Cow::Owned(result)
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

/// Compile un bloc de diagramme Mermaid avec un moteur spécifique et diagnostic détaillé.
///
/// # Errors
/// Renvoie `DiagramErrorDetail` si le moteur est indisponible ou la syntaxe invalide.
pub fn render_to_svg_detailed_with_engine(
    diagram: &DiagramBlock,
    theme_mode: ThemeMode,
    engine_type: DiagramEngineType,
) -> Result<String, DiagramErrorDetail> {
    let engine = get_engine(engine_type).map_err(|err| {
        DiagramErrorDetail::new(format!("{err}"), None, Some("EngineError".to_string()))
    })?;
    let sanitized = sanitize_mermaid_labels(diagram.as_str());
    let render_block = match sanitized {
        Cow::Owned(s) => DiagramBlock::with_metadata(s, diagram.metadata().clone()),
        Cow::Borrowed(_) => diagram.clone(),
    };
    engine.render_svg(&render_block, theme_mode)
}

/// Compile un bloc de diagramme Mermaid en document SVG vectoriel.
///
/// # Errors
/// Renvoie `CliError::MermaidSyntax` si la syntaxe Mermaid est invalide.
pub fn render_to_svg(diagram: &DiagramBlock, theme_mode: ThemeMode) -> Result<String, CliError> {
    render_to_svg_with_engine(diagram, theme_mode, DiagramEngineType::default())
}

/// Compile un bloc de diagramme Mermaid avec un moteur spécifique.
///
/// # Errors
/// Renvoie `CliError::MermaidSyntax` ou `CliError::CommandLine`.
pub fn render_to_svg_with_engine(
    diagram: &DiagramBlock,
    theme_mode: ThemeMode,
    engine_type: DiagramEngineType,
) -> Result<String, CliError> {
    render_to_svg_detailed_with_engine(diagram, theme_mode, engine_type).map_err(|detail| {
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

    #[test]
    fn test_engine_by_name_default() {
        let engine = engine_by_name("mermaid-svg");
        assert!(engine.is_ok());
        if let Ok(eng) = engine {
            assert_eq!(eng.name(), "mermaid-svg");
        }
    }

    #[test]
    fn test_engine_by_name_unknown() {
        let result = engine_by_name("unknown-engine");
        assert!(result.is_err());
    }

    #[cfg(feature = "merman")]
    fn merman_svg(source: &str, theme: ThemeMode) -> String {
        MermanEngine
            .render_svg(&DiagramBlock::new(source.to_string()), theme)
            .unwrap_or_default()
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_applies_requested_theme() {
        let source = "graph TD\nA-->B";
        let dark = merman_svg(source, ThemeMode::Dark);
        let light = merman_svg(source, ThemeMode::Light);
        assert!(dark.contains("<svg") && light.contains("<svg"));
        assert_ne!(dark, light);
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_user_init_directive_keeps_priority() {
        let source = "%%{init: {\"theme\": \"neutral\"}}%%\ngraph TD\nA-->B";
        assert_eq!(
            merman_svg(source, ThemeMode::Dark),
            merman_svg(source, ThemeMode::Light)
        );
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_theme_respects_frontmatter_position() {
        let source = "---\ntitle: Titre\n---\ngraph TD\nA-->B";
        let dark = merman_svg(source, ThemeMode::Dark);
        assert!(dark.contains("Titre"), "front matter perdu : {dark}");
        assert_ne!(dark, merman_svg(source, ThemeMode::Light));
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_theme_name_mapping() {
        assert_eq!(merman_theme_name(ThemeMode::Light), "default");
        assert_eq!(merman_theme_name(ThemeMode::Neutral), "neutral");
        for retro in [
            ThemeMode::Dark,
            ThemeMode::Amber,
            ThemeMode::Phosphor,
            ThemeMode::Neon,
            ThemeMode::Mono,
        ] {
            assert_eq!(merman_theme_name(retro), "dark");
        }
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_engine_by_name_merman() {
        let engine = engine_by_name("merman");
        assert!(engine.is_ok());
        if let Ok(eng) = engine {
            assert_eq!(eng.name(), "merman");
        }
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_render_flowchart() {
        let block = DiagramBlock::new("flowchart TD\n  A[Start] --> B[Done]".to_string());
        let engine = MermanEngine;
        let result = engine.render_svg(&block, ThemeMode::Dark);
        assert!(
            result.is_ok(),
            "Merman should render flowchart: {:?}",
            result.err()
        );
        let svg = result.unwrap_or_default();
        assert!(svg.contains("<svg"), "Output should contain SVG root tag");
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_render_sequence_diagram() {
        let block = DiagramBlock::new(
            "sequenceDiagram\n  autonumber\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi".to_string(),
        );
        let engine = MermanEngine;
        let result = engine.render_svg(&block, ThemeMode::Dark);
        assert!(
            result.is_ok(),
            "Merman should render sequence diagram: {:?}",
            result.err()
        );
        let svg = result.unwrap_or_default();
        assert!(svg.contains("<svg"), "Output should contain SVG root tag");
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_merman_render_state_diagram() {
        let block = DiagramBlock::new(
            "stateDiagram-v2\n  [*] --> Idle\n  Idle --> Processing\n  Processing --> [*]"
                .to_string(),
        );
        let engine = MermanEngine;
        let result = engine.render_svg(&block, ThemeMode::Dark);
        assert!(
            result.is_ok(),
            "Merman should render state diagram: {:?}",
            result.err()
        );
        let svg = result.unwrap_or_default();
        assert!(svg.contains("<svg"), "Output should contain SVG root tag");
    }

    #[test]
    fn test_sanitize_mermaid_labels_no_ampersand() {
        let src = "flowchart TD\n  A --> B";
        assert_eq!(sanitize_mermaid_labels(src), Cow::Borrowed(src));
    }

    #[test]
    fn test_sanitize_mermaid_labels_preserves_chaining_operator() {
        let src = "flowchart TD\n  A & B --> C & D";
        let sanitized = sanitize_mermaid_labels(src);
        assert_eq!(sanitized.as_ref(), "flowchart TD\n  A & B --> C & D");
    }

    #[test]
    fn test_sanitize_mermaid_labels_replaces_in_quoted_node() {
        let src = r#"flowchart TD
  C --> D["Filtrage & Sanctuarisation<br/>(ADR-0001, ADR-0002)"]
  D --> E["Consolidation & Livrable<br/>Rapport Markdown Complet"]"#;
        let sanitized = sanitize_mermaid_labels(src);
        assert!(sanitized.contains("Filtrage ﹠ Sanctuarisation"));
        assert!(sanitized.contains("Consolidation ﹠ Livrable"));
        assert!(!sanitized.contains("& "));
    }

    #[test]
    fn test_sanitize_mermaid_labels_replaces_entities() {
        let src = r#"flowchart TD
  A["A &amp; B"] --> C["C &#38; D"]"#;
        let sanitized = sanitize_mermaid_labels(src);
        assert!(sanitized.contains("A ﹠ B"));
        assert!(sanitized.contains("C ﹠ D"));
        assert!(!sanitized.contains("&amp;"));
        assert!(!sanitized.contains("&#38;"));
    }

    #[test]
    fn test_sanitize_mermaid_labels_preserves_html_entities() {
        let src = r#"flowchart TD
  A["x &lt; y and y &gt; z"]"#;
        let sanitized = sanitize_mermaid_labels(src);
        assert!(sanitized.contains("x &lt; y and y &gt; z"));
    }

    #[test]
    fn test_sanitize_mermaid_labels_edge_labels() {
        let src = r#"flowchart TD
  A -->|"Process & Validate"| B
  B -->|Step 1 & Step 2| C"#;
        let sanitized = sanitize_mermaid_labels(src);
        assert!(sanitized.contains(r#"|"Process ﹠ Validate"|"#));
        assert!(sanitized.contains("|Step 1 ﹠ Step 2|"));
    }

    #[test]
    fn test_render_exact_bug_screenshot_diagram() {
        let src = r#"flowchart TD
  A["Initialisation Audit<br/>& Outillage Qualité"] --> B["Collecte Statique<br/>PHPStan, Biome, Tests"]
  B --> C["Déclenchement 5 Piliers<br/>(Sec, Perf, Qual, QA, DBA)"]
  C --> D["Filtrage & Sanctuarisation<br/>(ADR-0001, ADR-0002)"]
  D --> E["Consolidation & Livrable<br/>Rapport Markdown Complet"]"#;
        let block = DiagramBlock::new(src.to_string());
        let svg = render_to_svg(&block, ThemeMode::Dark);
        assert!(svg.is_ok(), "Le diagramme issu du bug doit compiler en SVG");
        let svg_str = svg.unwrap_or_default();
        assert!(svg_str.contains("Initialisation Audit"));
        assert!(svg_str.contains("Filtrage"));
        assert!(svg_str.contains("Consolidation"));
    }
}
