//! Core table structures

use crate::Result;
use crate::error::TableError;
use crate::font::FontMetrics;
use crate::style::{CellStyle, RowStyle, TableStyle};
use std::sync::Arc;
use tracing::trace;

/// Image fit mode for cell rendering
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ImageFit {
    /// Scale to fit within cell bounds preserving aspect ratio
    #[default]
    Contain,
}

/// Text overlay drawn on top of a cell image (semi-transparent bar with white text).
#[derive(Debug, Clone)]
pub struct ImageOverlay {
    /// Text to display in the overlay bar
    pub text: String,
    /// Font size for the overlay text (default: 8.0)
    pub font_size: f32,
    /// Height of the semi-transparent background bar (default: 16.0)
    pub bar_height: f32,
    /// Horizontal padding inside the bar (default: 4.0)
    pub padding: f32,
}

impl ImageOverlay {
    /// Create a new overlay with the given text and sensible defaults.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: 8.0,
            bar_height: 16.0,
            padding: 4.0,
        }
    }
}

/// Image payload for embedding in a table cell.
///
/// Constructed from raw JPEG or PNG bytes. The image is validated and
/// converted to a PDF XObject stream at construction time. Cheap to
/// clone via internal `Arc`.
#[derive(Clone)]
pub struct CellImage {
    /// Pre-built XObject stream ready for PDF embedding
    pub(crate) xobject: Arc<lopdf::Stream>,
    /// Intrinsic width in pixels
    pub(crate) width_px: u32,
    /// Intrinsic height in pixels
    pub(crate) height_px: u32,
    /// Maximum rendered height in points (caps row height contribution)
    pub(crate) max_render_height_pts: Option<f32>,
    /// Fit mode
    pub(crate) fit: ImageFit,
    /// Optional text overlay drawn on top of the image
    pub(crate) overlay: Option<ImageOverlay>,
}

impl std::fmt::Debug for CellImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellImage")
            .field("width_px", &self.width_px)
            .field("height_px", &self.height_px)
            .field("max_render_height_pts", &self.max_render_height_pts)
            .field("fit", &self.fit)
            .field("overlay", &self.overlay)
            .finish()
    }
}

impl CellImage {
    /// Create a new image from raw JPEG or PNG bytes.
    ///
    /// Validates the image and pre-builds the PDF XObject stream.
    /// Returns an error if the bytes are not a valid/supported image.
    pub fn new(data: Vec<u8>) -> Result<Self> {
        let stream = lopdf::xobject::image_from(data)
            .map_err(|e| TableError::DrawingError(format!("Invalid image data: {e}")))?;

        let width_px = stream
            .dict
            .get(b"Width")
            .ok()
            .and_then(|o| match o {
                lopdf::Object::Integer(v) => Some(*v as u32),
                _ => None,
            })
            .ok_or_else(|| TableError::DrawingError("Missing image Width".into()))?;

        let height_px = stream
            .dict
            .get(b"Height")
            .ok()
            .and_then(|o| match o {
                lopdf::Object::Integer(v) => Some(*v as u32),
                _ => None,
            })
            .ok_or_else(|| TableError::DrawingError("Missing image Height".into()))?;

        Ok(Self {
            xobject: Arc::new(stream),
            width_px,
            height_px,
            max_render_height_pts: None,
            fit: ImageFit::default(),
            overlay: None,
        })
    }

    /// Set maximum rendered height in points.
    pub fn with_max_height(mut self, pts: f32) -> Self {
        self.max_render_height_pts = Some(pts);
        self
    }

    /// Set the image fit mode.
    pub fn with_fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    /// Attach a text overlay to this image.
    pub fn with_overlay(mut self, overlay: ImageOverlay) -> Self {
        self.overlay = Some(overlay);
        self
    }

    /// Intrinsic pixel width.
    pub fn width_px(&self) -> u32 {
        self.width_px
    }

    /// Intrinsic pixel height.
    pub fn height_px(&self) -> u32 {
        self.height_px
    }

    /// Aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        self.width_px as f32 / self.height_px as f32
    }
}

/// Column width specification
#[derive(Debug, Clone)]
pub enum ColumnWidth {
    /// Fixed width in points
    Pixels(f32),
    /// Percentage of available table width
    Percentage(f32),
    /// Automatically calculate based on content
    Auto,
}

/// Represents a table with rows and styling
#[derive(Clone)]
pub struct Table {
    pub rows: Vec<Row>,
    pub style: TableStyle,
    /// Column width specifications
    pub column_widths: Option<Vec<ColumnWidth>>,
    /// Total table width (if None, auto-calculate based on content)
    pub total_width: Option<f32>,
    /// Number of header rows to repeat on each page when paginating
    pub header_rows: usize,
    /// Font metrics for accurate text measurement and Unicode encoding.
    /// When set, enables font-aware text wrapping and glyph ID encoding.
    pub font_metrics: Option<Arc<dyn FontMetrics>>,
    /// Bold font metrics for accurate bold text measurement and Unicode encoding.
    /// When set, bold cells can use a dedicated embedded bold font.
    pub bold_font_metrics: Option<Arc<dyn FontMetrics>>,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field("rows", &self.rows)
            .field("style", &self.style)
            .field("column_widths", &self.column_widths)
            .field("total_width", &self.total_width)
            .field("header_rows", &self.header_rows)
            .field("font_metrics", &self.font_metrics.as_ref().map(|_| "..."))
            .field(
                "bold_font_metrics",
                &self.bold_font_metrics.as_ref().map(|_| "..."),
            )
            .finish()
    }
}

impl Table {
    /// Create a new empty table
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            style: TableStyle::default(),
            column_widths: None,
            total_width: None,
            header_rows: 0,
            font_metrics: None,
            bold_font_metrics: None,
        }
    }

    /// Add a row to the table
    pub fn add_row(mut self, row: Row) -> Self {
        trace!("Adding row with {} cells", row.cells.len());
        self.rows.push(row);
        self
    }

    /// Set the table style
    pub fn with_style(mut self, style: TableStyle) -> Self {
        self.style = style;
        self
    }

    /// Set column width specifications
    pub fn with_column_widths(mut self, widths: Vec<ColumnWidth>) -> Self {
        self.column_widths = Some(widths);
        self
    }

    /// Set total table width
    pub fn with_total_width(mut self, width: f32) -> Self {
        self.total_width = Some(width);
        self
    }

    /// Convenience method to set pixel widths for all columns
    pub fn with_pixel_widths(mut self, widths: Vec<f32>) -> Self {
        self.column_widths = Some(widths.into_iter().map(ColumnWidth::Pixels).collect());
        self
    }

    /// Set border width for the entire table
    pub fn with_border(mut self, width: f32) -> Self {
        self.style.border_width = width;
        self
    }

    /// Set the number of header rows to repeat on each page
    pub fn with_header_rows(mut self, count: usize) -> Self {
        self.header_rows = count;
        self
    }

    /// Set font metrics for accurate text measurement and Unicode encoding.
    ///
    /// When font metrics are provided along with `embedded_font_resource_name`
    /// on the table style, text will be encoded as glyph IDs and measured
    /// using the actual font data instead of heuristic estimates.
    pub fn with_font_metrics(mut self, metrics: impl FontMetrics + 'static) -> Self {
        self.font_metrics = Some(Arc::new(metrics));
        self
    }

    /// Set bold font metrics for accurate bold text measurement and Unicode encoding.
    ///
    /// When font metrics are provided along with `embedded_font_resource_name_bold`
    /// on the table style, bold cell text will be encoded as glyph IDs and measured
    /// using the bold font data.
    pub fn with_bold_font_metrics(mut self, metrics: impl FontMetrics + 'static) -> Self {
        self.bold_font_metrics = Some(Arc::new(metrics));
        self
    }

    /// Get the number of columns (based on the first row, accounting for colspan)
    pub fn column_count(&self) -> usize {
        self.rows
            .first()
            .map(|r| r.cells.iter().map(|c| c.colspan.max(1)).sum())
            .unwrap_or(0)
    }

    /// Validate table structure
    pub fn validate(&self) -> Result<()> {
        if self.rows.is_empty() {
            return Err(crate::error::TableError::InvalidTable(
                "Table has no rows".to_string(),
            ));
        }

        let expected_cols = self.column_count();
        for (i, row) in self.rows.iter().enumerate() {
            // Calculate the total column coverage including colspan
            let mut total_coverage = 0;
            for cell in &row.cells {
                total_coverage += cell.colspan.max(1);
            }

            if total_coverage != expected_cols {
                return Err(crate::error::TableError::InvalidTable(format!(
                    "Row {} covers {} columns (with colspan), expected {}",
                    i, total_coverage, expected_cols
                )));
            }
        }

        if let Some(ref widths) = self.column_widths {
            if widths.len() != expected_cols {
                return Err(crate::error::TableError::InvalidTable(format!(
                    "Column widths array has {} elements, but table has {} columns",
                    widths.len(),
                    expected_cols
                )));
            }

            // Check that percentage widths don't exceed 100%
            let total_percentage: f32 = widths
                .iter()
                .filter_map(|w| match w {
                    ColumnWidth::Percentage(p) => Some(*p),
                    _ => None,
                })
                .sum();

            if total_percentage > 100.0 {
                return Err(crate::error::TableError::InvalidTable(format!(
                    "Total percentage widths ({:.1}%) exceed 100%",
                    total_percentage
                )));
            }
        }

        Ok(())
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a row in a table
#[derive(Debug, Clone)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub style: Option<RowStyle>,
    /// Explicit height (if None, auto-calculate)
    pub height: Option<f32>,
}

impl Row {
    /// Create a new row with cells
    pub fn new(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            style: None,
            height: None,
        }
    }

    /// Set row style
    pub fn with_style(mut self, style: RowStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set explicit row height
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

/// Represents a cell in a table
#[derive(Debug, Clone)]
pub struct Cell {
    pub content: String,
    pub style: Option<CellStyle>,
    pub colspan: usize,
    pub rowspan: usize,
    /// Enable text wrapping for this cell
    pub text_wrap: bool,
    /// Image payloads for this cell (rendered side-by-side when multiple).
    pub images: Vec<CellImage>,
}

impl Cell {
    /// Create a new cell with text content
    pub fn new<S: Into<String>>(content: S) -> Self {
        Self {
            content: content.into(),
            style: None,
            colspan: 1,
            rowspan: 1,
            text_wrap: false,
            images: Vec::new(),
        }
    }

    /// Create an empty cell
    pub fn empty() -> Self {
        Self::new("")
    }

    /// Create a cell containing a single image (with empty text).
    pub fn from_image(image: CellImage) -> Self {
        Self {
            content: String::new(),
            style: None,
            colspan: 1,
            rowspan: 1,
            text_wrap: false,
            images: vec![image],
        }
    }

    /// Create a cell containing multiple images rendered side-by-side (with empty text).
    pub fn from_images(images: Vec<CellImage>) -> Self {
        Self {
            content: String::new(),
            style: None,
            colspan: 1,
            rowspan: 1,
            text_wrap: false,
            images,
        }
    }

    /// Set a single image payload for this cell (replaces any existing images).
    pub fn with_image(mut self, image: CellImage) -> Self {
        self.images = vec![image];
        self
    }

    /// Append an additional image to this cell.
    pub fn add_image(mut self, image: CellImage) -> Self {
        self.images.push(image);
        self
    }

    /// Enable text wrapping for this cell
    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.text_wrap = wrap;
        self
    }

    /// Set cell style
    pub fn with_style(mut self, style: CellStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set colspan
    pub fn with_colspan(mut self, span: usize) -> Self {
        self.colspan = span.max(1);
        self
    }

    /// Set rowspan
    pub fn with_rowspan(mut self, span: usize) -> Self {
        self.rowspan = span.max(1);
        self
    }

    /// Make text bold
    pub fn bold(mut self) -> Self {
        let mut style = self.style.unwrap_or_default();
        style.bold = true;
        self.style = Some(style);
        self
    }

    /// Make text italic
    pub fn italic(mut self) -> Self {
        let mut style = self.style.unwrap_or_default();
        style.italic = true;
        self.style = Some(style);
        self
    }

    /// Set font size
    pub fn with_font_size(mut self, size: f32) -> Self {
        let mut style = self.style.unwrap_or_default();
        style.font_size = Some(size);
        self.style = Some(style);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_validation() {
        let mut table = Table::new();
        assert!(table.validate().is_err());

        table = table.add_row(Row::new(vec![Cell::new("A"), Cell::new("B")]));
        assert!(table.validate().is_ok());

        table = table.add_row(Row::new(vec![Cell::new("C")]));
        assert!(table.validate().is_err());
    }

    #[test]
    fn test_cell_builder() {
        let cell = Cell::new("Test")
            .bold()
            .italic()
            .with_font_size(14.0)
            .with_colspan(2);

        assert_eq!(cell.content, "Test");
        assert_eq!(cell.colspan, 2);
        let style = cell.style.unwrap();
        assert!(style.bold);
        assert!(style.italic);
        assert_eq!(style.font_size, Some(14.0));
    }

    #[test]
    fn test_cell_font_name() {
        // Test with custom font
        let style = CellStyle {
            font_name: Some("Courier".to_string()),
            ..Default::default()
        };
        let cell = Cell::new("Monospace text").with_style(style);

        assert_eq!(cell.content, "Monospace text");
        let cell_style = cell.style.unwrap();
        assert_eq!(cell_style.font_name, Some("Courier".to_string()));

        // Test with default (no font specified)
        let cell_default = Cell::new("Default font");
        assert!(cell_default.style.is_none());
    }

    #[test]
    fn test_with_bold_font_metrics_builder() {
        struct DummyMetrics;

        impl crate::font::FontMetrics for DummyMetrics {
            fn char_width(&self, _ch: char, _font_size: f32) -> f32 {
                5.0
            }

            fn text_width(&self, text: &str, _font_size: f32) -> f32 {
                text.chars().count() as f32 * 5.0
            }

            fn encode_text(&self, text: &str) -> Vec<u8> {
                vec![0; text.chars().count() * 2]
            }
        }

        let table = Table::new().with_bold_font_metrics(DummyMetrics);
        assert!(table.bold_font_metrics.is_some());
    }
}
