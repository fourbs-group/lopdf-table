//! Core table structures

use crate::Result;
use crate::style::{CellStyle, RowStyle, TableStyle};
use tracing::trace;

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
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: Vec<Row>,
    pub style: TableStyle,
    /// Column width specifications
    pub column_widths: Option<Vec<ColumnWidth>>,
    /// Total table width (if None, auto-calculate based on content)
    pub total_width: Option<f32>,
    /// Number of header rows to repeat on each page when paginating
    pub header_rows: usize,
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
        }
    }

    /// Create an empty cell
    pub fn empty() -> Self {
        Self::new("")
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
}
