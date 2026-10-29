use crate::domain::RasterizedImage;
use std::fmt::Write;
use std::ops::Range;

/// Opacité au-delà de laquelle un pixel est tracé ; en deçà, la cellule reste vide.
const VISIBLE_ALPHA_THRESHOLD: u8 = 64;

/// Taille moyenne observée d'une cellule encodée, pour préallouer la sortie.
const ESTIMATED_BYTES_PER_CELL: usize = 24;

/// Pixel RGBA 8 bits par canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgba {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Rgba {
    const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    const fn is_visible(self) -> bool {
        self.alpha > VISIBLE_ALPHA_THRESHOLD
    }
}

/// Somme des canaux d'un ensemble de pixels, avant moyennage.
#[derive(Debug, Default, Clone, Copy)]
struct ChannelSums {
    red: u64,
    green: u64,
    blue: u64,
    alpha: u64,
    count: u64,
}

impl ChannelSums {
    fn with(self, pixel: Rgba) -> Self {
        Self {
            red: self.red + u64::from(pixel.red),
            green: self.green + u64::from(pixel.green),
            blue: self.blue + u64::from(pixel.blue),
            alpha: self.alpha + u64::from(pixel.alpha),
            count: self.count + 1,
        }
    }

    /// Moyenne par canal ; transparent si aucun pixel n'a été cumulé.
    fn mean(self) -> Rgba {
        if self.count == 0 {
            return Rgba::TRANSPARENT;
        }
        let average = |sum: u64| u8::try_from(sum / self.count).unwrap_or(0);
        Rgba::new(
            average(self.red),
            average(self.green),
            average(self.blue),
            average(self.alpha),
        )
    }
}

/// Rectangle de pixels `[x.start, x.end) × [y.start, y.end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PixelRegion {
    x: Range<u32>,
    y: Range<u32>,
}

impl PixelRegion {
    /// Ramène la région dans l'image en lui garantissant au moins un pixel.
    fn clamped_to(&self, image: &RasterizedImage) -> Self {
        Self {
            x: clamp_to_length(&self.x, image.width),
            y: clamp_to_length(&self.y, image.height),
        }
    }
}

fn clamp_to_length(span: &Range<u32>, length: u32) -> Range<u32> {
    let start = span.start.min(length.saturating_sub(1));
    let end = span.end.max(span.start.saturating_add(1)).min(length);
    start..end
}

/// Tranche `index` (sur `parts`) d'un axe de `length` pixels, bornes arrondies vers l'extérieur.
fn scaled_span(index: u32, parts: u32, length: u32) -> Range<u32> {
    let start = (index * length) / parts;
    let end = ((index + 1) * length).div_ceil(parts);
    start..end
}

fn pixel_at(image: &RasterizedImage, x: u32, y: u32) -> Option<Rgba> {
    let index = (y as usize * image.width as usize + x as usize) * 4;
    match image.rgba.get(index..index + 4)? {
        &[red, green, blue, alpha] => Some(Rgba::new(red, green, blue, alpha)),
        _ => None,
    }
}

/// Moyenne les pixels de `region`, bornée à l'image.
fn sample_pixel_area(image: &RasterizedImage, region: &PixelRegion) -> Rgba {
    let region = region.clamped_to(image);
    region
        .y
        .flat_map(|y| region.x.clone().map(move |x| (x, y)))
        .filter_map(|(x, y)| pixel_at(image, x, y))
        .fold(ChannelSums::default(), ChannelSums::with)
        .mean()
}

/// Deux demi-lignes de pixels superposées, tracées dans une seule cellule de terminal.
#[derive(Debug, Clone, Copy)]
struct HalfBlockCell {
    top: Rgba,
    bottom: Rgba,
}

/// Formate un caractère demi-bloc avec couleurs ANSI `TrueColor` 24-bit.
fn format_halfblock_cell(out: &mut String, cell: HalfBlockCell) {
    let HalfBlockCell { top, bottom } = cell;
    let _ = match (top.is_visible(), bottom.is_visible()) {
        (true, true) => write!(
            out,
            "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
            top.red, top.green, top.blue, bottom.red, bottom.green, bottom.blue
        ),
        (true, false) => write!(
            out,
            "\x1b[38;2;{};{};{}m\x1b[49m▀",
            top.red, top.green, top.blue
        ),
        (false, true) => write!(
            out,
            "\x1b[38;2;{};{};{}m\x1b[49m▄",
            bottom.red, bottom.green, bottom.blue
        ),
        (false, false) => write!(out, "\x1b[0m "),
    };
}

/// Grille de cellules de demi-blocs plaquée sur une image.
struct HalfBlockGrid<'a> {
    image: &'a RasterizedImage,
    cols: u32,
    rows: u32,
}

impl<'a> HalfBlockGrid<'a> {
    /// Grille d'au plus `target_cols` colonnes ; `None` si l'image ou la largeur est vide.
    fn fit(image: &'a RasterizedImage, target_cols: u16) -> Option<Self> {
        if image.width == 0 || image.height == 0 || target_cols == 0 {
            return None;
        }
        let max_cols = u16::try_from(image.width).unwrap_or(u16::MAX);
        let cols = u32::from(target_cols.min(max_cols));
        let pixel_height = (cols * image.height) / image.width;
        Some(Self {
            image,
            cols,
            rows: pixel_height.div_ceil(2),
        })
    }

    fn capacity_hint(&self) -> usize {
        self.rows as usize * self.cols as usize * ESTIMATED_BYTES_PER_CELL
    }

    fn cell(&self, col: u32, row: u32) -> HalfBlockCell {
        let x = scaled_span(col, self.cols, self.image.width);
        let half_rows = self.rows * 2;
        let sample_half_row = |half_row: u32| {
            let y = scaled_span(half_row, half_rows, self.image.height);
            sample_pixel_area(self.image, &PixelRegion { x: x.clone(), y })
        };
        HalfBlockCell {
            top: sample_half_row(row * 2),
            bottom: sample_half_row(row * 2 + 1),
        }
    }

    fn write_row(&self, out: &mut String, row: u32) {
        for col in 0..self.cols {
            format_halfblock_cell(out, self.cell(col, row));
        }
        out.push_str("\x1b[0m\n");
    }
}

/// Convertit une image matricielle en texte ANSI avec demi-blocs Unicode (`▀`, `▄`).
#[must_use]
pub fn encode_halfblocks(image: &RasterizedImage, target_cols: u16) -> String {
    let Some(grid) = HalfBlockGrid::fit(image, target_cols) else {
        return String::new();
    };
    let mut output = String::with_capacity(grid.capacity_hint());
    for row in 0..grid.rows {
        grid.write_row(&mut output, row);
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

    fn column_image(top: [u8; 4], bottom: [u8; 4]) -> RasterizedImage {
        RasterizedImage::new(1, 2, [top, bottom].concat())
    }

    #[test]
    fn test_encode_halfblocks_both_halves_visible_sets_foreground_and_background() {
        let image = column_image([255, 0, 0, 255], [0, 0, 255, 255]);
        assert_eq!(
            encode_halfblocks(&image, 1),
            "\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀\x1b[0m\n"
        );
    }

    #[test]
    fn test_encode_halfblocks_only_top_visible_uses_upper_half_block() {
        let image = column_image([0, 255, 0, 255], [9, 9, 9, 0]);
        assert_eq!(
            encode_halfblocks(&image, 1),
            "\x1b[38;2;0;255;0m\x1b[49m▀\x1b[0m\n"
        );
    }

    #[test]
    fn test_encode_halfblocks_only_bottom_visible_uses_lower_half_block() {
        let image = column_image([9, 9, 9, 0], [0, 255, 255, 255]);
        assert_eq!(
            encode_halfblocks(&image, 1),
            "\x1b[38;2;0;255;255m\x1b[49m▄\x1b[0m\n"
        );
    }

    #[test]
    fn test_encode_halfblocks_fully_transparent_cell_is_blank() {
        let image = column_image([1, 2, 3, 0], [4, 5, 6, 0]);
        assert_eq!(encode_halfblocks(&image, 1), "\x1b[0m \x1b[0m\n");
    }

    #[test]
    fn test_encode_halfblocks_alpha_threshold_is_exclusive_at_64() {
        let hidden = column_image([7, 7, 7, 64], [7, 7, 7, 64]);
        let visible = column_image([7, 7, 7, 65], [7, 7, 7, 64]);
        assert_eq!(encode_halfblocks(&hidden, 1), "\x1b[0m \x1b[0m\n");
        assert_eq!(
            encode_halfblocks(&visible, 1),
            "\x1b[38;2;7;7;7m\x1b[49m▀\x1b[0m\n"
        );
    }

    #[test]
    fn test_encode_halfblocks_averages_pixels_of_each_half() {
        let rgba = vec![
            200, 0, 0, 255, 100, 0, 0, 255, //
            0, 40, 0, 255, 0, 60, 0, 255,
        ];
        let image = RasterizedImage::new(2, 2, rgba);
        assert_eq!(
            encode_halfblocks(&image, 1),
            "\x1b[38;2;150;0;0m\x1b[48;2;0;50;0m▀\x1b[0m\n"
        );
    }

    #[test]
    fn test_encode_halfblocks_never_exceeds_image_width() {
        let image = column_image([255, 0, 0, 255], [0, 0, 255, 255]);
        assert_eq!(encode_halfblocks(&image, 10), encode_halfblocks(&image, 1));
    }

    #[test]
    fn test_scaled_span_partitions_length_without_gap() {
        assert_eq!(scaled_span(0, 2, 5), 0..3);
        assert_eq!(scaled_span(1, 2, 5), 2..5);
    }

    fn two_by_two_image() -> RasterizedImage {
        RasterizedImage::new(
            2,
            2,
            vec![
                10, 20, 30, 255, 30, 40, 50, 255, //
                50, 60, 70, 255, 70, 80, 90, 255,
            ],
        )
    }

    #[test]
    fn test_sample_pixel_area_averages_region() {
        let region = PixelRegion { x: 0..2, y: 0..2 };
        assert_eq!(
            sample_pixel_area(&two_by_two_image(), &region),
            Rgba::new(40, 50, 60, 255)
        );
    }

    #[test]
    fn test_sample_pixel_area_clamps_out_of_bounds_region_to_last_pixel() {
        let region = PixelRegion { x: 9..12, y: 9..12 };
        assert_eq!(
            sample_pixel_area(&two_by_two_image(), &region),
            Rgba::new(70, 80, 90, 255)
        );
    }

    #[test]
    fn test_sample_pixel_area_widens_empty_region_to_one_pixel() {
        let region = PixelRegion { x: 1..1, y: 0..0 };
        assert_eq!(
            sample_pixel_area(&two_by_two_image(), &region),
            Rgba::new(30, 40, 50, 255)
        );
    }

    #[test]
    fn test_sample_pixel_area_ignores_truncated_pixel_buffer() {
        let image = RasterizedImage::new(2, 1, vec![10, 20, 30, 255]);
        let region = PixelRegion { x: 0..2, y: 0..1 };
        assert_eq!(
            sample_pixel_area(&image, &region),
            Rgba::new(10, 20, 30, 255)
        );
    }

    #[test]
    fn test_sample_pixel_area_of_zero_sized_image_is_transparent() {
        let image = RasterizedImage::new(0, 0, Vec::new());
        let region = PixelRegion { x: 0..1, y: 0..1 };
        assert_eq!(sample_pixel_area(&image, &region), Rgba::TRANSPARENT);
    }

    #[test]
    fn test_encode_halfblocks_empty_inputs_yield_empty_text() {
        let image = column_image([255, 0, 0, 255], [0, 0, 255, 255]);
        assert_eq!(encode_halfblocks(&image, 0), "");
        assert_eq!(
            encode_halfblocks(&RasterizedImage::new(0, 4, Vec::new()), 4),
            ""
        );
        assert_eq!(
            encode_halfblocks(&RasterizedImage::new(4, 0, Vec::new()), 4),
            ""
        );
    }
}
