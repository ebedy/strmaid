use crate::domain::{CliError, RasterizedImage};
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{Options, Transform, Tree};

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
    let opt = Options::default();
    let tree =
        Tree::from_str(svg_data, &opt).map_err(|err| CliError::SvgParsing(format!("{err:?}")))?;

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

    #[test]
    fn test_rasterize_rejects_oversized_svg() {
        let svg = "<svg width=\"200\" height=\"100\"><rect width=\"200\" height=\"100\" fill=\"blue\"/></svg>";
        let img = rasterize_svg(svg, 200, 1);

        assert!(matches!(img, Err(CliError::ResourceLimit(_))));
    }
}
