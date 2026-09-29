//! The size of a bitmap of `spython.image`, from the header of the image in
//! its data URI. The page and the native run read it the same way.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use sinteract::asset::image_head;

/// The width and the height of the image in `data_uri`, which are the size
/// of the original on the screen, or the message of the error.
pub(crate) fn bitmap_size(data_uri: &str) -> Result<(f64, f64), String> {
    let blob = decode_data_uri(data_uri).ok_or("a bitmap needs a data URI in base64")?;
    let head = image_head(&blob).map_err(|e| e.to_string())?;
    Ok((f64::from(head.width), f64::from(head.height)))
}

/// The bytes of a `data:` URI in base64, or `None` when `uri` is not one.
pub(crate) fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let (header, payload) = uri.strip_prefix("data:")?.split_once(',')?;
    header.ends_with(";base64").then_some(())?;
    B64.decode(payload).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signature and the start of the IHDR chunk of a PNG of 3 by 2.
    const HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x03\0\0\0\x02";

    #[test]
    fn the_size_of_a_bitmap_comes_from_its_header() {
        let uri = format!("data:image/png;base64,{}", B64.encode(HEADER));
        assert_eq!(decode_data_uri(&uri).as_deref(), Some(HEADER));
        assert_eq!(bitmap_size(&uri), Ok((3.0, 2.0)));
    }

    #[test]
    fn what_is_not_an_image_has_no_size() {
        assert!(bitmap_size("data:image/png;base64,AAAA").is_err());
        assert!(bitmap_size("data:image/png,abc").is_err());
    }

    #[test]
    fn decode_data_uri_refuses_a_uri_that_is_not_base64() {
        assert_eq!(decode_data_uri("data:image/png,abc"), None);
        assert_eq!(decode_data_uri("image/png;base64,abc"), None);
    }
}
