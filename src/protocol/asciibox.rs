use crate::domain::DiagramErrorDetail;
use crate::mermaid::split_frontmatter;
use mermaid_text::{
    Error as MermaidTextError, RenderOptions as MtRenderOptions, render_with_options,
};
use std::borrow::Cow;

/// Ouvertures de forme de nœud pouvant précéder un libellé entre guillemets.
const LABEL_OPENERS: [char; 6] = ['[', '(', '{', '>', '/', '\\'];
/// Fermetures de forme de nœud pouvant suivre un libellé entre guillemets.
const LABEL_CLOSERS: [char; 5] = [']', ')', '}', '/', '\\'];
/// Terminaisons de flèche pouvant précéder un libellé d'arête `|…|`.
const ARROW_ENDINGS: [&str; 9] = ["->", "=>", "--", "==", "-o", "-x", "=o", "=x", ".-"];

/// Options de configuration pour le rendu textuel `AsciiBox`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiBoxOptions {
    pub target_cols: u16,
    pub color: bool,
}

impl AsciiBoxOptions {
    #[must_use]
    pub const fn new(target_cols: u16, color: bool) -> Self {
        Self { target_cols, color }
    }
}

/// Rendu sémantique d'un diagramme Mermaid en texte Unicode Box-Drawing calibré au terminal.
///
/// # Errors
/// Renvoie `DiagramErrorDetail` si la syntaxe Mermaid est invalide ou non supportée, ou
/// (`kind: TooWide`) si le rendu dépasse `options.target_cols` même après compactage.
pub fn render_asciibox(
    source: &str,
    options: AsciiBoxOptions,
) -> Result<String, DiagramErrorDetail> {
    let budget = usize::from(options.target_cols.max(10));
    let opts = MtRenderOptions {
        max_width: Some(budget),
        max_width_strict: true,
        color: options.color,
        ascii: false,
        ..MtRenderOptions::default()
    };

    let (_frontmatter, body) = split_frontmatter(source);
    render_with_options(&adapt_for_mermaid_text(body), &opts).map_err(map_mermaid_text_error)
}

/// Compense deux écarts de `mermaid-text` 0.57 avec Mermaid sur les flowcharts : les
/// guillemets d'un libellé de nœud (`A["texte"]`) sont affichés tels quels, et une arête
/// `A --> |texte| B` est lue comme un nœud faute de libellé collé à la flèche.
fn adapt_for_mermaid_text(source: &str) -> Cow<'_, str> {
    if !is_flowchart(source) {
        return Cow::Borrowed(source);
    }
    let lines: Vec<Cow<'_, str>> = source.split_inclusive('\n').map(adapt_line).collect();
    if lines.iter().all(|line| matches!(line, Cow::Borrowed(_))) {
        return Cow::Borrowed(source);
    }
    Cow::Owned(lines.concat())
}

/// En-tête `flowchart` ou `graph`, après un éventuel front matter YAML et les lignes `%%`.
fn is_flowchart(source: &str) -> bool {
    let mut lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"));
    let header = match lines.next() {
        Some("---") => lines.skip_while(|line| *line != "---").nth(1),
        first => first,
    };
    header
        .and_then(|line| line.split_whitespace().next())
        .is_some_and(|keyword| matches!(keyword, "flowchart" | "flowchart-elk" | "graph"))
}

fn adapt_line(line: &str) -> Cow<'_, str> {
    if line.trim_start().starts_with("%%") || !line.contains(['"', '|']) {
        return Cow::Borrowed(line);
    }
    let mut scanner = LineScanner::default();
    let mut rest = line;
    while let Some(ch) = rest.chars().next() {
        rest = &rest[scanner.step(rest, ch)..];
    }
    if scanner.out == line {
        return Cow::Borrowed(line);
    }
    Cow::Owned(scanner.out)
}

/// Réécriture d'une ligne de flowchart ; `depth` compte les formes de nœud ouvertes.
#[derive(Default)]
struct LineScanner {
    out: String,
    depth: usize,
}

impl LineScanner {
    /// Consomme le début de `rest`, qui commence par `ch`, et renvoie la longueur lue en octets.
    fn step(&mut self, rest: &str, ch: char) -> usize {
        match ch {
            '"' => self.quoted(rest),
            ' ' | '\t' => self.spacing(rest),
            _ => {
                self.track_depth(ch);
                self.out.push(ch);
                ch.len_utf8()
            }
        }
    }

    /// Recopie une chaîne entre guillemets, sans eux lorsqu'elle forme le libellé entier.
    fn quoted(&mut self, rest: &str) -> usize {
        let Some(close) = rest[1..].find('"').map(|offset| offset + 1) else {
            self.out.push_str(rest);
            return rest.len();
        };
        let is_whole_label =
            self.out.ends_with(LABEL_OPENERS) && rest[close + 1..].starts_with(LABEL_CLOSERS);
        let kept = if is_whole_label && parses_unquoted(&rest[1..close]) {
            &rest[1..close]
        } else {
            &rest[..=close]
        };
        self.out.push_str(kept);
        close + 1
    }

    /// Supprime les blancs entre une flèche et son libellé `|…|`, hors forme de nœud.
    fn spacing(&mut self, rest: &str) -> usize {
        let blank = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let joins_label = self.depth == 0
            && rest[blank..].starts_with('|')
            && ARROW_ENDINGS.iter().any(|arrow| self.out.ends_with(arrow));
        if !joins_label {
            self.out.push_str(&rest[..blank]);
        }
        blank
    }

    fn track_depth(&mut self, ch: char) {
        self.depth = bracket_depth(self.depth, ch).unwrap_or(0);
    }
}

/// Libellé lisible par `mermaid-text` sans guillemets : pas de séparateur d'instructions `;`
/// et des délimiteurs équilibrés (`A[a { b]` est sinon lu comme un nœud brut).
fn parses_unquoted(label: &str) -> bool {
    !label.contains(';') && label.chars().try_fold(0, bracket_depth) == Some(0)
}

/// Profondeur de délimiteurs après `ch`, `None` sur une fermeture sans ouverture.
fn bracket_depth(depth: usize, ch: char) -> Option<usize> {
    match ch {
        '[' | '(' | '{' => Some(depth + 1),
        ']' | ')' | '}' => depth.checked_sub(1),
        _ => Some(depth),
    }
}

fn map_mermaid_text_error(err: MermaidTextError) -> DiagramErrorDetail {
    match err {
        MermaidTextError::EmptyInput => DiagramErrorDetail::new(
            "diagramme Mermaid vide".to_string(),
            Some(1),
            Some("EmptyInput".to_string()),
        ),
        MermaidTextError::UnsupportedDiagram(diag) => DiagramErrorDetail::new(
            format!("type de diagramme non supporté en mode texte: {diag}"),
            Some(1),
            Some("UnsupportedDiagram".to_string()),
        ),
        MermaidTextError::ParseError(msg) => DiagramErrorDetail::new(
            format!("erreur de syntaxe Mermaid: {msg}"),
            None,
            Some("ParseError".to_string()),
        ),
        MermaidTextError::TooWide { requested, actual } => DiagramErrorDetail::new(
            format!(
                "le diagramme dépasse la largeur du terminal ({actual} > {requested} colonnes)"
            ),
            None,
            Some("TooWide".to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::display_width;

    #[test]
    fn test_render_asciibox_valid_flowchart() {
        let opts = AsciiBoxOptions::new(80, false);
        let result = render_asciibox("graph LR\n  A[Build] --> B[Deploy]", opts);
        assert!(result.is_ok());
        let rendered = result.unwrap_or_default();
        assert!(rendered.contains("Build"));
        assert!(rendered.contains("Deploy"));
        assert!(rendered.chars().any(|c| c == '┌' || c == '│' || c == '└'));
    }

    #[test]
    fn test_render_asciibox_ignores_leading_frontmatter() {
        let opts = AsciiBoxOptions::new(80, false);
        let source = "---\ntitle: Demo\n---\nflowchart TD\n    A[Start] --> B[End]";
        let rendered = render_asciibox(source, opts).unwrap_or_default();
        assert!(rendered.contains("Start"), "rendu absent : {rendered:?}");
        assert!(rendered.contains("End"));
    }

    #[test]
    fn test_render_asciibox_frontmatter_matches_plain_rendering() {
        let opts = AsciiBoxOptions::new(80, false);
        let body = "sequenceDiagram\n    Alice->>Bob: Hello";
        let with_frontmatter = render_asciibox(&format!("---\ntitle: Demo\n---\n{body}"), opts);
        assert_eq!(with_frontmatter, render_asciibox(body, opts));
        assert!(with_frontmatter.is_ok());
    }

    #[test]
    fn test_render_asciibox_unclosed_frontmatter_is_not_stripped() {
        let opts = AsciiBoxOptions::new(80, false);
        let result = render_asciibox("---\ntitle: Demo\nflowchart TD\n    A --> B", opts);
        assert!(result.is_err());
    }

    const WIDTH_SOURCES: [&str; 5] = [
        "graph LR\n  Alpha --> Beta --> Gamma --> Delta --> Epsilon --> Zeta",
        "sequenceDiagram\n  participant Développeur\n  participant Agent\n  participant Outil\n  Développeur->>Agent: Prompt avec demande de schéma\n  Agent->>Outil: Flux Markdown",
        "gantt\n  title Planification du projet\n  dateFormat YYYY-MM-DD\n  section Phase\n  Tâche une :a1, 2026-01-01, 30d",
        "timeline\n  title Historique des versions\n  2024 : Une première version\n  2025 : Une deuxième version",
        "pie title Répartition\n  \"Un libellé assez long\" : 40\n  \"Un autre libellé\" : 60",
    ];

    #[test]
    fn test_render_asciibox_never_exceeds_requested_width() {
        for source in WIDTH_SOURCES {
            for cols in [20_u16, 40, 80] {
                let Ok(rendered) = render_asciibox(source, AsciiBoxOptions::new(cols, false))
                else {
                    continue;
                };
                let widest = rendered.lines().map(display_width).max().unwrap_or(0);
                assert!(
                    widest <= usize::from(cols),
                    "{widest} > {cols} colonnes pour {source:?}"
                );
            }
        }
    }

    #[test]
    fn test_render_asciibox_too_wide_reports_dedicated_error() {
        let source = WIDTH_SOURCES[0];
        let err = render_asciibox(source, AsciiBoxOptions::new(20, false))
            .err()
            .unwrap_or_else(|| DiagramErrorDetail::new(String::new(), None, None));
        assert_eq!(err.kind.as_deref(), Some("TooWide"), "{err:?}");
        assert!(err.message.contains("20"), "{}", err.message);
    }

    #[test]
    fn test_render_asciibox_empty_input_returns_error() {
        let opts = AsciiBoxOptions::new(80, false);
        let result = render_asciibox("   ", opts);
        assert!(result.is_err());
        let err = result
            .err()
            .unwrap_or_else(|| DiagramErrorDetail::new(String::new(), None, None));
        assert_eq!(err.kind.as_deref(), Some("EmptyInput"));
    }

    #[test]
    fn test_adapt_for_mermaid_text_unquotes_every_node_shape() {
        let cases = [
            (r#"A["Rect"]"#, "A[Rect]"),
            (r#"A("Rond")"#, "A(Rond)"),
            (r#"A{"Losange"}"#, "A{Losange}"),
            (r#"A(["Stade"])"#, "A([Stade])"),
            (r#"A[("Cylindre")]"#, "A[(Cylindre)]"),
            (r#"A[["Routine"]]"#, "A[[Routine]]"),
            (r#"A{{"Hexagone"}}"#, "A{{Hexagone}}"),
            (r#"A(("Cercle"))"#, "A((Cercle))"),
            (r#"A>"Drapeau"]"#, "A>Drapeau]"),
            (r#"A[/"Para"/]"#, "A[/Para/]"),
            (
                r#"A["Deux<br/>lignes & f(x) [1]"]"#,
                "A[Deux<br/>lignes & f(x) [1]]",
            ),
            (r#"A["x --> |y"]"#, "A[x --> |y]"),
        ];
        for (node, expected) in cases {
            let source = format!("flowchart TD\n  {node} --> B");
            assert_eq!(
                adapt_for_mermaid_text(&source),
                format!("flowchart TD\n  {expected} --> B"),
                "{node}"
            );
        }
    }

    #[test]
    fn test_adapt_for_mermaid_text_joins_pipe_label_to_arrow() {
        let cases = [
            (r#"A --> |"lien"| B"#, r#"A -->|"lien"| B"#),
            ("A -.-> |pointillé| B", "A -.->|pointillé| B"),
            ("A ==>   |épais| B", "A ==>|épais| B"),
            ("A --- |plat| B", "A ---|plat| B"),
        ];
        for (edge, expected) in cases {
            let source = format!("graph LR\n  {edge}");
            assert_eq!(
                adapt_for_mermaid_text(&source),
                format!("graph LR\n  {expected}"),
                "{edge}"
            );
        }
    }

    #[test]
    fn test_adapt_for_mermaid_text_leaves_other_syntax_untouched() {
        let untouched = [
            "flowchart TD\n  A[Sans guillemets] --> B",
            "flowchart TD\n  A[Il dit \"oui\"] --> B",
            "flowchart TD\n  A -->|\"déjà collé\"| B",
            "flowchart TD\n  A[\"a; b\"] --> B",
            "flowchart TD\n  A[\"a { b\"] --> B",
            "flowchart TD\n  A[\"a ) b\"] --> B",
            "flowchart TD\n  A[x --> |y] --> B",
            "flowchart TD\n  %% A[\"commentaire\"] --> |z| B\n  A --> B",
            "stateDiagram-v2\n  state \"Long nom\" as S\n  [*] --> S",
            "sequenceDiagram\n  A->>B: \"citation\"",
        ];
        for source in untouched {
            assert!(
                matches!(adapt_for_mermaid_text(source), Cow::Borrowed(_)),
                "{source}"
            );
        }
    }

    #[test]
    fn test_adapt_for_mermaid_text_detects_flowchart_after_preamble() {
        let source = "---\ntitle: \"Titre\"\n---\n%%{init: {}}%%\nflowchart TD\n  A[\"x\"] --> B";
        assert_eq!(
            adapt_for_mermaid_text(source),
            "---\ntitle: \"Titre\"\n---\n%%{init: {}}%%\nflowchart TD\n  A[x] --> B"
        );
    }

    #[test]
    fn test_render_asciibox_hides_label_quotes() {
        let opts = AsciiBoxOptions::new(80, false);
        let source = "flowchart TD\n  A[\"Un libellé\"] --> |\"lien\"| B(\"Rond\")";
        let rendered = render_asciibox(source, opts).unwrap_or_default();
        assert!(rendered.contains("Un libellé"), "{rendered}");
        assert!(rendered.contains("lien"), "{rendered}");
        assert!(!rendered.contains('"'), "{rendered}");
        assert!(!rendered.contains('|'), "{rendered}");
    }

    #[test]
    fn test_render_asciibox_compacts_for_narrow_width() {
        let opts_wide = AsciiBoxOptions::new(120, false);
        let opts_narrow = AsciiBoxOptions::new(40, false);
        let source = "graph LR\n  Alpha --> Beta --> Gamma";
        let wide = render_asciibox(source, opts_wide).unwrap_or_default();
        let narrow = render_asciibox(source, opts_narrow).unwrap_or_default();
        assert_ne!(wide, "");
        assert_ne!(narrow, "");
        let max_narrow_line = narrow.lines().map(str::len).max().unwrap_or(0);
        let max_wide_line = wide.lines().map(str::len).max().unwrap_or(0);
        assert!(max_narrow_line <= max_wide_line);
    }
}
