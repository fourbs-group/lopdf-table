//! Core table structures

use crate::Result;
use crate::style::{CellStyle, RowStyle, TableStyle};
use tracing::trace;

/// Represents a table with rows and styling
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: Vec<Row>,
    pub style: TableStyle,
    /// Explicit column widths (if None, auto-calculate)
    pub column_widths: Option<Vec<f32>>,
}

impl Table {
    /// Create a new empty table
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            style: TableStyle::default(),
            column_widths: None,
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

    /// Set explicit column widths
    pub fn with_column_widths(mut self, widths: Vec<f32>) -> Self {
        self.column_widths = Some(widths);
        self
    }

    /// Set border width for the entire table
    pub fn with_border(mut self, width: f32) -> Self {
        self.style.border_width = width;
        self
    }

    /// Get the number of columns (based on the first row)
    pub fn column_count(&self) -> usize {
        self.rows.first().map(|r| r.cells.len()).unwrap_or(0)
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
            if row.cells.len() != expected_cols {
                return Err(crate::error::TableError::InvalidTable(format!(
                    "Row {} has {} cells, expected {}",
                    i,
                    row.cells.len(),
                    expected_cols
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
}

impl Cell {
    /// Create a new cell with text content
    pub fn new<S: Into<String>>(content: S) -> Self {
        Self {
            content: content.into(),
            style: None,
            colspan: 1,
            rowspan: 1,
        }
    }

    /// Create an empty cell
    pub fn empty() -> Self {
        Self::new("")
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
}
