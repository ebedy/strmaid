use crate::domain::RasterizedImage;
use base64::prelude::*;
use std::fmt::Write;

const CHUNK_SIZE: usize = 4096;

/// Encode une image matricielle au format Kitty Graphics Protocol.
#[must_use]
pub fn encode_kitty_graphics(image: &RasterizedImage) -> String {
    let b64 = BASE64_STANDARD.encode(&image.rgba);
    let total_len = b64.len();
    let mut output = String::with_capacity(total_len + 256);

    let mut offset = 0;
    let mut is_first = true;

    while offset < total_len {
        let end = (offset + CHUNK_SIZE).min(total_len);
        let chunk = &b64[offset..end];
        let more = i32::from(end < total_len);

        if is_first {
            let width = image.width;
            let height = image.height;
            let _ = write!(
                output,
                "\x1b_Ga=T,f=32,s={width},v={height},m={more};{chunk}\x1b\\"
            );
            is_first = false;
        } else {
            let _ = write!(output, "\x1b_Gm={more};{chunk}\x1b\\");
        }

        offset = end;
    }

    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_kitty_small_image() {
        let image = RasterizedImage::new(2, 2, vec![255; 16]);
        let encoded = encode_kitty_graphics(&image);
        assert!(encoded.starts_with("\x1b_Ga=T,f=32,s=2,v=2,m=0;"));
        assert!(encoded.ends_with("\x1b\\\n"));
    }
}
