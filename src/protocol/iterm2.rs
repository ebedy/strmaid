use crate::domain::{CliError, RasterizedImage};
use base64::prelude::*;
use resvg::tiny_skia::PixmapRef;

/// Encode une image matricielle au format iTerm2 Inline Images (séquence OSC 1337).
///
/// # Errors
/// Renvoie `CliError::ImageEncoding` si le buffer RGBA ou les dimensions sont invalides,
/// ou si l'encodage PNG en mémoire échoue.
pub fn encode_iterm2(image: &RasterizedImage) -> Result<String, CliError> {
    if image.width == 0 || image.height == 0 {
        return Ok(String::new());
    }

    let pixmap_ref = PixmapRef::from_bytes(&image.rgba, image.width, image.height)
        .ok_or_else(|| CliError::ImageEncoding("Dimensions ou buffer RGBA invalide".to_string()))?;

    let png_bytes = pixmap_ref
        .encode_png()
        .map_err(|e| CliError::ImageEncoding(format!("Échec encodage PNG: {e}")))?;

    let payload = BASE64_STANDARD.encode(&png_bytes);
    Ok(format!(
        "\x1b]1337;File=inline=1;width=auto;height=auto;preserveAspectRatio=1:{payload}\x07\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_iterm2_empty_image() {
        let image = RasterizedImage::new(0, 0, vec![]);
        let result = encode_iterm2(&image);
        assert!(result.is_ok());
        let encoded = result.unwrap_or_default();
        assert_eq!(encoded, "");
    }

    #[test]
    fn test_encode_iterm2_small_image() {
        // 2x2 pixels RGBA valides (16 octets)
        let image = RasterizedImage::new(2, 2, vec![255; 16]);
        let result = encode_iterm2(&image);
        assert!(result.is_ok());
        let encoded = result.unwrap_or_default();
        assert!(
            encoded.starts_with(
                "\x1b]1337;File=inline=1;width=auto;height=auto;preserveAspectRatio=1:"
            )
        );
        assert!(encoded.ends_with("\x07\n"));
    }

    #[test]
    fn test_encode_iterm2_invalid_buffer() {
        // Buffer plus court que width * height * 4
        let image = RasterizedImage::new(2, 2, vec![255; 8]);
        let result = encode_iterm2(&image);
        assert!(result.is_err());
    }
}
