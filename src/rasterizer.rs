use crate::domain::{CliError, RasterizedImage};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::fontdb::Database;
use resvg::usvg::{Options, Transform, Tree};
use std::sync::{Arc, LazyLock};

/// Familles sans-serif privilégiées, par ordre de préférence, pour le générique `sans-serif`.
const PREFERRED_SANS_SERIF_FAMILIES: [&str; 6] = [
    "Arial",
    "Helvetica",
    "DejaVu Sans",
    "Liberation Sans",
    "Noto Sans",
    "Segoe UI",
];

/// Base de polices système chargée une seule fois, au premier rendu graphique.
static FONT_DATABASE: LazyLock<Arc<Database>> = LazyLock::new(|| Arc::new(load_font_database()));

fn load_font_database() -> Database {
    let mut database = Database::new();
    database.load_system_fonts();
    if let Some(family) = resolve_sans_serif_family(&database) {
        database.set_sans_serif_family(family);
    }
    database
}

/// Famille réellement installée à associer au générique `sans-serif` : la première
/// famille privilégiée disponible, sinon la première famille de la base.
fn resolve_sans_serif_family(database: &Database) -> Option<String> {
    let has_family = |name: &str| {
        database
            .faces()
            .any(|face| face.families.iter().any(|(family, _)| family == name))
    };
    PREFERRED_SANS_SERIF_FAMILIES
        .into_iter()
        .find(|name| has_family(name))
        .map(str::to_string)
        .or_else(|| {
            database
                .faces()
                .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
        })
}

/// Nombre de faces de polices système chargées (diagnostic `strmaid doctor`).
#[must_use]
pub fn loaded_font_count() -> usize {
    FONT_DATABASE.len()
}

/// Analyse le SVG avec la base de polices fournie.
///
/// Une base vide ferait disparaître silencieusement les libellés `<text>` : l'erreur
/// `CliError::FontUnavailable` déclenche alors le repli `AsciiBox` du renderer.
fn parse_svg_tree(svg_data: &str, fonts: &Arc<Database>) -> Result<Tree, CliError> {
    if fonts.is_empty() && svg_data.contains("<text") {
        return Err(CliError::FontUnavailable(
            "aucune police système chargée pour les libellés du diagramme".to_string(),
        ));
    }
    let options = Options {
        font_family: "sans-serif".to_string(),
        fontdb: Arc::clone(fonts),
        ..Options::default()
    };
    Tree::from_str(svg_data, &options).map_err(|err| CliError::SvgParsing(format!("{err:?}")))
}

/// Calcule les dimensions cibles en conservant le ratio d'aspect.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn compute_scaled_dimensions(
    orig_w: f32,
    orig_h: f32,
    target_width_px: u32,
) -> (u32, u32, Transform) {
    if orig_w <= 0.0 || orig_h <= 0.0 {
        return (100, 100, Transform::default());
    }

    let target_w_f32 = target_width_px as f32;
    let scale = if target_width_px > 0 && orig_w > target_w_f32 {
        target_w_f32 / orig_w
    } else {
        1.0
    };

    let final_w = (orig_w * scale).round().max(1.0) as u32;
    let final_h = (orig_h * scale).round().max(1.0) as u32;
    let transform = Transform::from_scale(scale, scale);

    (final_w, final_h, transform)
}

/// Rasterise un contenu SVG en une image matricielle RGBA.
///
/// # Errors
/// Renvoie `CliError::SvgParsing` ou `CliError::PixmapAllocation`.
pub fn rasterize_svg(
    svg_data: &str,
    target_width_px: u32,
    max_pixels: u32,
) -> Result<RasterizedImage, CliError> {
    let tree = parse_svg_tree(svg_data, &FONT_DATABASE)?;

    let size = tree.size();
    let (width, height, transform) =
        compute_scaled_dimensions(size.width(), size.height(), target_width_px);
    let pixels = width.saturating_mul(height);
    if pixels > max_pixels {
        return Err(CliError::ResourceLimit(format!(
            "rasterisation {width}x{height} ({pixels} pixels) supérieure à {max_pixels} pixels"
        )));
    }

    let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
        CliError::PixmapAllocation(format!("Impossible d'allouer {width}x{height} pixels"))
    })?;

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(RasterizedImage::new(width, height, pixmap.data().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ResourceLimits;

    #[test]
    fn test_rasterize_valid_svg() {
        let svg = "<svg width=\"200\" height=\"100\"><rect width=\"200\" height=\"100\" fill=\"blue\"/></svg>";
        let img = rasterize_svg(svg, 100, ResourceLimits::default().max_raster_pixels);
        assert!(img.is_ok());
        let res = img.unwrap_or_else(|_| RasterizedImage::new(0, 0, Vec::new()));
        assert_eq!(res.width, 100);
        assert_eq!(res.height, 50);
        assert_eq!(res.rgba.len(), (100 * 50 * 4));
    }

    const TEXT_SVG_TEMPLATE: &str = "<svg width=\"200\" height=\"60\"><text x=\"10\" y=\"40\" font-family=\"sans-serif\" font-size=\"30\" fill=\"black\">LABEL</text></svg>";

    #[test]
    fn test_parse_svg_tree_without_fonts_reports_font_unavailable() {
        let empty = Arc::new(Database::new());
        let tree = parse_svg_tree(TEXT_SVG_TEMPLATE, &empty);
        assert!(matches!(tree, Err(CliError::FontUnavailable(_))));
    }

    #[test]
    fn test_parse_svg_tree_without_fonts_accepts_shapes_only() {
        let empty = Arc::new(Database::new());
        let svg = "<svg width=\"20\" height=\"10\"><rect width=\"20\" height=\"10\"/></svg>";
        assert!(parse_svg_tree(svg, &empty).is_ok());
    }

    #[test]
    fn test_resolve_sans_serif_family_on_empty_database() {
        assert_eq!(resolve_sans_serif_family(&Database::new()), None);
    }

    #[test]
    fn test_rasterize_text_labels_produce_distinct_pixels() {
        let render = |label: &str| {
            let svg = TEXT_SVG_TEMPLATE.replace("LABEL", label);
            rasterize_svg(&svg, 200, ResourceLimits::default().max_raster_pixels)
        };
        let (first, second) = (render("AAAA"), render("BBBB"));
        if loaded_font_count() == 0 {
            assert!(matches!(first, Err(CliError::FontUnavailable(_))));
            return;
        }
        assert!(first.is_ok() && second.is_ok());
        let first = first.map(|img| img.rgba).unwrap_or_default();
        let second = second.map(|img| img.rgba).unwrap_or_default();
        assert!(
            first != second,
            "les libellés doivent produire des glyphes distincts"
        );
    }

    #[test]
    fn test_rasterize_rejects_oversized_svg() {
        let svg = "<svg width=\"200\" height=\"100\"><rect width=\"200\" height=\"100\" fill=\"blue\"/></svg>";
        let img = rasterize_svg(svg, 200, 1);

        assert!(matches!(img, Err(CliError::ResourceLimit(_))));
    }
}
