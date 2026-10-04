use crate::domain::RasterizedImage;

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
}
