use crate::domain::RasterizedImage;
use std::fmt::Write;

/// Échantillonne un pixel RGBA aux coordonnées spécifiées.
fn sample_pixel_area(
    image: &RasterizedImage,
    x_start: u32,
    x_end: u32,
    y_start: u32,
    y_end: u32,
) -> (u8, u8, u8, u8) {
    if image.width == 0 || image.height == 0 {
        return (0, 0, 0, 0);
    }

    let x0 = x_start.min(image.width.saturating_sub(1));
    let x1 = x_end.max(x_start + 1).min(image.width);
    let y0 = y_start.min(image.height.saturating_sub(1));
    let y1 = y_end.max(y_start + 1).min(image.height);

    let mut red = 0_u64;
    let mut green = 0_u64;
    let mut blue = 0_u64;
    let mut alpha = 0_u64;
    let mut count = 0_u64;

    for y in y0..y1 {
        for x in x0..x1 {
            let idx = ((y * image.width + x) * 4) as usize;
            if idx + 3 < image.rgba.len() {
                red += u64::from(image.rgba[idx]);
                green += u64::from(image.rgba[idx + 1]);
                blue += u64::from(image.rgba[idx + 2]);
                alpha += u64::from(image.rgba[idx + 3]);
                count += 1;
            }
        }
    }

    let avg = |sum: u64| -> u8 {
        sum.checked_div(count)
            .and_then(|v| u8::try_from(v).ok())
            .unwrap_or(0)
    };

    (avg(red), avg(green), avg(blue), avg(alpha))
}

/// Formate un caractère demi-bloc avec couleurs ANSI `TrueColor` 24-bit.
fn format_halfblock_cell(out: &mut String, top: (u8, u8, u8, u8), bot: (u8, u8, u8, u8)) {
    let top_visible = top.3 > 64;
    let bot_visible = bot.3 > 64;

    match (top_visible, bot_visible) {
        (true, true) => {
            let _ = write!(
                out,
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                top.0, top.1, top.2, bot.0, bot.1, bot.2
            );
        }
        (true, false) => {
            let _ = write!(out, "\x1b[38;2;{};{};{}m\x1b[49m▀", top.0, top.1, top.2);
        }
        (false, true) => {
            let _ = write!(out, "\x1b[38;2;{};{};{}m\x1b[49m▄", bot.0, bot.1, bot.2);
        }
        (false, false) => out.push_str("\x1b[0m "),
    }
}

/// Convertit une image matricielle en texte ANSI avec demi-blocs Unicode (`▀`, `▄`).
#[must_use]
pub fn encode_halfblocks(image: &RasterizedImage, target_cols: u16) -> String {
    if image.width == 0 || image.height == 0 || target_cols == 0 {
        return String::new();
    }

    let max_cols = u16::try_from(image.width).unwrap_or(u16::MAX);
    let cols = u32::from(target_cols.min(max_cols));
    let pixel_height = (cols * image.height) / image.width;
    let rows = pixel_height.div_ceil(2);

    let mut output = String::with_capacity((rows * cols * 24) as usize);

    for r in 0..rows {
        for c in 0..cols {
            let x_start = (c * image.width) / cols;
            let x_end = ((c + 1) * image.width).div_ceil(cols);
            let y_top_start = (r * 2 * image.height) / (rows * 2);
            let y_top_end = ((r * 2 + 1) * image.height).div_ceil(rows * 2);
            let y_bot_start = ((r * 2 + 1) * image.height) / (rows * 2);
            let y_bot_end = ((r * 2 + 2) * image.height).div_ceil(rows * 2);

            let top = sample_pixel_area(image, x_start, x_end, y_top_start, y_top_end);
            let bot = sample_pixel_area(image, x_start, x_end, y_bot_start, y_bot_end);

            format_halfblock_cell(&mut output, top, bot);
        }
        output.push_str("\x1b[0m\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_halfblocks_non_empty() {
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let image = RasterizedImage::new(2, 2, rgba);
        let text = encode_halfblocks(&image, 2);
        assert_ne!(text, "");
        assert!(text.contains('▀') || text.contains('▄'));
    }
}
