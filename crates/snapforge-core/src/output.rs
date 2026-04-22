pub fn detect_image_format(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        "webp"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else {
        "png"
    }
}

pub fn build_capture_filename(ext: &str, timestamp: &str) -> String {
    format!("Gaze_{timestamp}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_png_format() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_image_format(&png_header), "png");
    }

    #[test]
    fn detect_webp_format() {
        let mut webp_header = Vec::new();
        webp_header.extend_from_slice(b"RIFF");
        webp_header.extend_from_slice(&[0x00; 4]);
        webp_header.extend_from_slice(b"WEBP");
        assert_eq!(detect_image_format(&webp_header), "webp");
    }

    #[test]
    fn detect_jpeg_format() {
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_image_format(&jpeg_header), "jpeg");
    }

    #[test]
    fn detect_unknown_format_falls_back_to_png() {
        assert_eq!(detect_image_format(b"RIFF1234"), "png");
        assert_eq!(detect_image_format(&[]), "png");
        assert_eq!(detect_image_format(&[0x00, 0x01, 0x02]), "png");
    }

    #[test]
    fn build_capture_filename_for_png() {
        assert_eq!(
            build_capture_filename("png", "20260327_120000"),
            "Gaze_20260327_120000.png"
        );
    }

    #[test]
    fn build_capture_filename_for_webp() {
        assert_eq!(
            build_capture_filename("webp", "20260327_120000"),
            "Gaze_20260327_120000.webp"
        );
    }
}
