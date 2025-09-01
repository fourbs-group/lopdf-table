//! Styling structures for tables, rows, and cells

use crate::constants::DEFAULT_MARGIN;

/// RGB color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    /// Create a new RGB color (values should be 0.0-1.0)
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
        }
    }

    /// Black color
    pub fn black() -> Self {
        Self::rgb(0.0, 0.0, 0.0)
    }

    /// White color
    pub fn white() -> Self {
        Self::rgb(1.0, 1.0, 1.0)
    }

    /// Gray color
    pub fn gray(level: f32) -> Self {
        let l = level.clamp(0.0, 1.0);
        Self::rgb(l, l, l)
    }

    /// Light gray
    pub fn light_gray() -> Self {
        Self::gray(0.8)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

impl Default for Alignment {
    fn default() -> Self {
        Self::Left
    }
}

/// Vertical alignment options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalAlignment {
    Top,
    Middle,
    Bottom,
}

impl Default for VerticalAlignment {
    fn default() -> Self {
        Self::Middle
    }
}

/// Border style options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self::Solid
    }
}

/// Padding for cells
#[derive(Debug, Clone, Copy)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    /// Create uniform padding
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create padding with vertical and horizontal values
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            bottom: vertical,
            left: horizontal,
            right: horizontal,
        }
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::uniform(5.0)
    }
}

/// Styling for the entire table
#[derive(Debug, Clone)]
pub struct TableStyle {
    pub border_style: BorderStyle,
    pub border_width: f32,
    pub border_color: Color,
    pub background_color: Option<Color>,
    pub padding: Padding,
    /// Default font for the table
    pub font_name: String,
    pub default_font_size: f32,
    /// Page height for pagination (if None, uses standard A4: 842 points)
    pub page_height: Option<f32>,
    /// Top margin for pages
    pub top_margin: f32,
    /// Bottom margin for pages
    pub bottom_margin: f32,
    /// Whether to repeat header rows on new pages
    pub repeat_headers: bool,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            border_style: BorderStyle::Solid,
            border_width: 1.0,
            border_color: Color::black(),
            background_color: None,
            padding: Padding::default(),
            font_name: "Helvetica".to_string(),
            default_font_size: 10.0,
            page_height: None, // Will default to A4 (842 points)
            top_margin: DEFAULT_MARGIN,
            bottom_margin: DEFAULT_MARGIN,
            repeat_headers: true,
        }
    }
}

/// Styling for a row
#[derive(Debug, Clone)]
pub struct RowStyle {
    pub background_color: Option<Color>,
    pub border_top: Option<(BorderStyle, f32, Color)>,
    pub border_bottom: Option<(BorderStyle, f32, Color)>,
    pub height: Option<f32>,
}

impl Default for RowStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            border_top: None,
            border_bottom: None,
            height: None,
        }
    }
}

/// Styling for a cell
#[derive(Debug, Clone)]
pub struct CellStyle {
    pub background_color: Option<Color>,
    pub text_color: Color,
    pub font_size: Option<f32>,
    /// Font name for this cell. If None, inherits from table's font_name.
    /// Supported fonts: "Helvetica", "Courier", "Times-Roman" (and their bold variants)
    pub font_name: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub alignment: Alignment,
    pub vertical_alignment: VerticalAlignment,
    pub padding: Option<Padding>,
    pub border_left: Option<(BorderStyle, f32, Color)>,
    pub border_right: Option<(BorderStyle, f32, Color)>,
    pub border_top: Option<(BorderStyle, f32, Color)>,
    pub border_bottom: Option<(BorderStyle, f32, Color)>,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            text_color: Color::black(),
            font_size: None,
            font_name: None,
            bold: false,
            italic: false,
            alignment: Alignment::Left,
            vertical_alignment: VerticalAlignment::Middle,
            padding: None,
            border_left: None,
            border_right: None,
            border_top: None,
            border_bottom: None,
        }
    }
}

impl CellStyle {
    /// Create a header cell style (bold, centered)
    pub fn header() -> Self {
        Self {
            bold: true,
            alignment: Alignment::Center,
            background_color: Some(Color::light_gray()),
            ..Default::default()
        }
    }
}
