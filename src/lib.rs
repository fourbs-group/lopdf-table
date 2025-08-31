//! A composable table drawing library for PDFs built on lopdf
//!
//! This library provides an ergonomic API for creating tables in PDF documents
//! with support for automatic sizing, custom styling, and flexible layouts.

use lopdf::{Document, Object, ObjectId};
use tracing::{debug, instrument, trace};

mod drawing;
pub mod error;
pub mod layout;
pub mod style;
pub mod table;

pub use error::{Result, TableError};
pub use style::{Alignment, BorderStyle, CellStyle, Color, RowStyle, TableStyle};
pub use table::{Cell, Row, Table};

/// Extension trait for lopdf::Document to add table drawing capabilities
pub trait TableDrawing {
    /// Draw a table at the specified position on a page
    ///
    /// # Arguments
    /// * `page_id` - The object ID of the page to draw on
    /// * `table` - The table to draw
    /// * `position` - The (x, y) position of the table's top-left corner
    ///
    /// # Returns
    /// Returns Ok(()) on success, or an error if the table cannot be drawn
    fn draw_table(&mut self, page_id: ObjectId, table: Table, position: (f32, f32)) -> Result<()>;

    /// Add a table to a page with automatic positioning
    ///
    /// This method will find an appropriate position on the page for the table
    fn add_table_to_page(&mut self, page_id: ObjectId, table: Table) -> Result<()>;

    /// Create table content operations without adding to document
    ///
    /// Useful for custom positioning or combining with other content
    fn create_table_content(&self, table: &Table, position: (f32, f32)) -> Result<Vec<Object>>;
}

impl TableDrawing for Document {
    #[instrument(skip(self, table), fields(table_rows = table.rows.len()))]
    fn draw_table(&mut self, page_id: ObjectId, table: Table, position: (f32, f32)) -> Result<()> {
        debug!("Drawing table at position {:?}", position);

        // Calculate layout
        let layout = layout::calculate_layout(&table)?;
        trace!("Calculated layout: {:?}", layout);

        // Generate drawing operations
        let operations = drawing::generate_table_operations(&table, &layout, position)?;

        // Add content to page
        drawing::add_operations_to_page(self, page_id, operations)?;

        Ok(())
    }

    #[instrument(skip(self, table))]
    fn add_table_to_page(&mut self, page_id: ObjectId, table: Table) -> Result<()> {
        // For now, default to top-left with some margin
        let position = (50.0, 750.0);
        self.draw_table(page_id, table, position)
    }

    fn create_table_content(&self, table: &Table, position: (f32, f32)) -> Result<Vec<Object>> {
        let layout = layout::calculate_layout(table)?;
        drawing::generate_table_operations(table, &layout, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_table_creation() {
        let table = Table::new()
            .add_row(Row::new(vec![Cell::new("Header 1"), Cell::new("Header 2")]))
            .add_row(Row::new(vec![Cell::new("Data 1"), Cell::new("Data 2")]));

        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].cells.len(), 2);
    }
}
