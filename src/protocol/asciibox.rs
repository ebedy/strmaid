use crate::domain::DiagramErrorDetail;
use mermaid_text::{
    Error as MermaidTextError, RenderOptions as MtRenderOptions, render_with_options,
};

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
/// Renvoie `DiagramErrorDetail` si la syntaxe Mermaid est invalide ou non supportée.
pub fn render_asciibox(
    source: &str,
    options: AsciiBoxOptions,
) -> Result<String, DiagramErrorDetail> {
    let budget = usize::from(options.target_cols.max(10));
    let opts = MtRenderOptions {
        max_width: Some(budget),
        max_width_strict: false,
        color: options.color,
        ascii: false,
        ..MtRenderOptions::default()
    };

    render_with_options(source, &opts).map_err(map_mermaid_text_error)
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
