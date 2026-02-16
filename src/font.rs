//! Font metrics for accurate text measurement and encoding

#[cfg(feature = "ttf-parser")]
use crate::constants::DEFAULT_CHAR_WIDTH_RATIO;

/// Trait for measuring text dimensions and encoding text for PDF rendering.
///
/// Implement this trait to provide accurate font-aware text measurement
/// and glyph encoding for Unicode text rendering with embedded fonts.
pub trait FontMetrics {
    /// Width of a single character in points at the given font size
    fn char_width(&self, ch: char, font_size: f32) -> f32;

    /// Total width of a string in points at the given font size
    fn text_width(&self, text: &str, font_size: f32) -> f32;

    /// Encode text for the PDF Tj operator (e.g., 2-byte big-endian glyph IDs for Type0 fonts)
    fn encode_text(&self, text: &str) -> Vec<u8>;
}

/// TrueType font metrics using ttf-parser for accurate glyph measurement and encoding.
///
/// This struct owns the font data and parses it on demand for measurements.
/// The caller is responsible for embedding the font into the PDF document;
/// this type only handles measurement and glyph ID encoding.
#[cfg(feature = "ttf-parser")]
pub struct TtfFontMetrics {
    font_data: Vec<u8>,
    units_per_em: f32,
}

#[cfg(feature = "ttf-parser")]
impl TtfFontMetrics {
    /// Create new font metrics from raw TTF/TTC font data.
    ///
    /// Validates the font by parsing it and extracting units_per_em.
    pub fn new(font_data: Vec<u8>) -> crate::Result<Self> {
        let face = ttf_parser::Face::parse(&font_data, 0).map_err(|e| {
            crate::error::TableError::TextError(format!("Failed to parse font: {e}"))
        })?;
        let units_per_em = face.units_per_em() as f32;
        Ok(Self {
            font_data,
            units_per_em,
        })
    }
}

#[cfg(feature = "ttf-parser")]
impl FontMetrics for TtfFontMetrics {
    fn char_width(&self, ch: char, font_size: f32) -> f32 {
        let face = ttf_parser::Face::parse(&self.font_data, 0).unwrap();
        face.glyph_index(ch)
            .and_then(|gid| face.glyph_hor_advance(gid))
            .map(|advance| advance as f32 / self.units_per_em * font_size)
            .unwrap_or(font_size * DEFAULT_CHAR_WIDTH_RATIO)
    }

    fn text_width(&self, text: &str, font_size: f32) -> f32 {
        let face = ttf_parser::Face::parse(&self.font_data, 0).unwrap();
        text.chars()
            .map(|ch| {
                face.glyph_index(ch)
                    .and_then(|gid| face.glyph_hor_advance(gid))
                    .map(|advance| advance as f32 / self.units_per_em * font_size)
                    .unwrap_or(font_size * DEFAULT_CHAR_WIDTH_RATIO)
            })
            .sum()
    }

    fn encode_text(&self, text: &str) -> Vec<u8> {
        let face = ttf_parser::Face::parse(&self.font_data, 0).unwrap();
        let mut bytes = Vec::with_capacity(text.len() * 2);
        for ch in text.chars() {
            let glyph_id = face.glyph_index(ch).map(|g| g.0).unwrap_or(0);
            bytes.extend_from_slice(&glyph_id.to_be_bytes());
        }
        bytes
    }
}

#[cfg(feature = "ttf-parser")]
impl std::fmt::Debug for TtfFontMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TtfFontMetrics")
            .field("units_per_em", &self.units_per_em)
            .field("font_data_len", &self.font_data.len())
            .finish()
    }
}

#[cfg(test)]
#[cfg(feature = "ttf-parser")]
mod tests {
    use super::*;

    fn load_test_font() -> Option<Vec<u8>> {
        // Try common system font paths
        let paths = [
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ];
        for path in &paths {
            if let Ok(data) = std::fs::read(path) {
                return Some(data);
            }
        }
        None
    }

    #[test]
    fn test_ttf_font_metrics_invalid_data() {
        let result = TtfFontMetrics::new(vec![0, 1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn test_ttf_font_metrics_valid_font() {
        let Some(font_data) = load_test_font() else {
            eprintln!("Skipping test: no system font found");
            return;
        };
        let metrics = TtfFontMetrics::new(font_data).expect("should parse system font");
        assert!(metrics.units_per_em > 0.0);
    }

    #[test]
    fn test_char_width_returns_positive() {
        let Some(font_data) = load_test_font() else {
            return;
        };
        let metrics = TtfFontMetrics::new(font_data).unwrap();
        let w = metrics.char_width('A', 12.0);
        assert!(w > 0.0, "char_width should be positive, got {w}");
    }

    #[test]
    fn test_text_width_sums_correctly() {
        let Some(font_data) = load_test_font() else {
            return;
        };
        let metrics = TtfFontMetrics::new(font_data).unwrap();
        let single = metrics.char_width('A', 12.0);
        let triple = metrics.text_width("AAA", 12.0);
        assert!(
            (triple - single * 3.0).abs() < 0.01,
            "text_width should equal sum of char_widths"
        );
    }

    #[test]
    fn test_encode_text_produces_two_bytes_per_char() {
        let Some(font_data) = load_test_font() else {
            return;
        };
        let metrics = TtfFontMetrics::new(font_data).unwrap();
        let encoded = metrics.encode_text("ABC");
        assert_eq!(
            encoded.len(),
            6,
            "3 chars should produce 6 bytes (2 per glyph ID)"
        );
    }

    #[test]
    fn test_encode_text_unicode() {
        let Some(font_data) = load_test_font() else {
            return;
        };
        let metrics = TtfFontMetrics::new(font_data).unwrap();
        let encoded = metrics.encode_text("café");
        assert_eq!(encoded.len(), 8, "4 chars should produce 8 bytes");
    }
}
