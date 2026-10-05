use crate::domain::{DiagramErrorDetail, RasterizedImage};
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

const DOT_OFFSETS: [(u32, u32, u8); 8] = [
    (0, 0, 0x01),
    (0, 1, 0x02),
    (0, 2, 0x04),
    (0, 3, 0x40),
    (1, 0, 0x08),
    (1, 1, 0x10),
    (1, 2, 0x20),
    (1, 3, 0x80),
];

/// Encode une image matricielle en tracé textuel Unicode Braille 2×4 sans couleur.
#[must_use]
pub fn encode_asciibox(image: &RasterizedImage, target_cols: u16) -> String {
    if image.width == 0 || image.height == 0 || target_cols == 0 {
        return String::new();
    }

    let max_cols = u16::try_from(image.width).unwrap_or(u16::MAX);
    let cols = u32::from(target_cols.min(max_cols).max(1));
    let rows = ((cols * image.height) / (image.width * 2)).max(1);
    let total_dots = (cols * 2, rows * 4);
    let bg_lum = detect_background_luminance(image);

    let mut output = String::with_capacity((rows * (cols + 1)) as usize);
    for row in 0..rows {
        render_asciibox_row(&mut output, image, row, cols, total_dots, bg_lum);
        output.push('\n');
    }
    output
}

/// Rendu d'une ligne de cellules de texte Braille.
fn render_asciibox_row(
    output: &mut String,
    image: &RasterizedImage,
    row: u32,
    cols: u32,
    total_dots: (u32, u32),
    bg_lum: u8,
) {
    for col in 0..cols {
        let mask = compute_cell_mask(image, (col, row), total_dots, bg_lum);
        output.push(mask_to_char(mask));
    }
}

/// Calcule le masque binaire Braille 2×4 pour une cellule donnée.
fn compute_cell_mask(
    image: &RasterizedImage,
    cell: (u32, u32),
    total_dots: (u32, u32),
    bg_lum: u8,
) -> u8 {
    let (col, row) = cell;
    let (total_x, total_y) = total_dots;
    let mut mask = 0_u8;
    for (dx, dy, bit) in DOT_OFFSETS {
        let px = (col * 2 + dx) * image.width / total_x;
        let py = (row * 4 + dy) * image.height / total_y;
        if is_dot_active(image, px, py, bg_lum) {
            mask |= bit;
        }
    }
    mask
}

/// Convertit un masque binaire 8-bit en caractère Unicode Braille ou espace.
const fn mask_to_char(mask: u8) -> char {
    if mask == 0 {
        return ' ';
    }
    match char::from_u32(0x2800 + mask as u32) {
        Some(c) => c,
        None => ' ',
    }
}

/// Évalue si un sous-point présente un contraste suffisant avec l'arrière-plan.
fn is_dot_active(image: &RasterizedImage, px: u32, py: u32, bg_lum: u8) -> bool {
    let lum = sample_pixel_luminance(image, px, py);
    lum.abs_diff(bg_lum) > 35
}

/// Détecte la luminance de l'arrière-plan à partir du pixel supérieur gauche.
fn detect_background_luminance(image: &RasterizedImage) -> u8 {
    if image.width == 0 || image.height == 0 || image.rgba.len() < 4 {
        return 0;
    }
    sample_pixel_luminance(image, 0, 0)
}

/// Échantillonne la luminance perçue d'un pixel RGBA.
fn sample_pixel_luminance(image: &RasterizedImage, coord_x: u32, coord_y: u32) -> u8 {
    let px = coord_x.min(image.width.saturating_sub(1));
    let py = coord_y.min(image.height.saturating_sub(1));
    let idx = ((py * image.width + px) * 4) as usize;
    if idx + 3 >= image.rgba.len() {
        return 0;
    }
    let alpha = image.rgba[idx + 3];
    if alpha < 64 {
        return 0;
    }
    let red = u32::from(image.rgba[idx]);
    let green = u32::from(image.rgba[idx + 1]);
    let blue = u32::from(image.rgba[idx + 2]);
    u8::try_from((red * 54 + green * 183 + blue * 19) / 256).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_asciibox_empty_image() {
        let image = RasterizedImage::new(0, 0, Vec::new());
        let output = encode_asciibox(&image, 80);
        assert_eq!(output, "");
    }

    #[test]
    fn test_encode_asciibox_zero_cols() {
        let image = RasterizedImage::new(10, 10, vec![0; 400]);
        let output = encode_asciibox(&image, 0);
        assert_eq!(output, "");
    }

    #[test]
    fn test_encode_asciibox_uniform_background_outputs_spaces() {
        // Image uniforme sombre 10x10
        let image = RasterizedImage::new(10, 10, vec![30; 400]);
        let output = encode_asciibox(&image, 5);
        assert_ne!(output, "");
        for line in output.lines() {
            assert!(line.chars().all(|c| c == ' '));
        }
    }

    #[test]
    fn test_encode_asciibox_with_contrasting_dots() {
        // Image 4x8 avec fond noir et un pixel blanc au centre
        let mut rgba = vec![0_u8; 4 * 8 * 4];
        // Met tous les pixels en alpha=255
        for i in (3..rgba.len()).step_by(4) {
            rgba[i] = 255;
        }
        // Pixel blanc à x=2, y=4
        let idx = (4 * 4 + 2) * 4;
        rgba[idx] = 255;
        rgba[idx + 1] = 255;
        rgba[idx + 2] = 255;

        let image = RasterizedImage::new(4, 8, rgba);
        let output = encode_asciibox(&image, 2);

        assert_ne!(output, "");
        // Au moins un caractère doit être un point Braille (non espace)
        assert!(output.chars().any(|c| c != ' ' && c != '\n'));
    }

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
