//! A composable table drawing library for PDFs built on lopdf
//!
//! This library provides an ergonomic API for creating tables in PDF documents
//! with support for automatic sizing, custom styling, and flexible layouts.

use lopdf::{Document, Object, ObjectId};
use tracing::{debug, instrument, trace};

mod constants;
mod drawing;
mod drawing_utils;
pub mod error;
pub mod font;
pub mod layout;
pub mod style;
pub mod table;
mod text;

// Re-export constants for public use
pub use constants::*;

pub use error::{Result, TableError};
pub use font::FontMetrics;
#[cfg(feature = "ttf-parser")]
pub use font::TtfFontMetrics;
pub use style::{
    Alignment, BorderStyle, CellStyle, Color, RowStyle, TableStyle, VerticalAlignment,
};
pub use table::{Cell, ColumnWidth, Row, Table};

/// Result of drawing a paginated table
#[derive(Debug, Clone)]
pub struct PagedTableResult {
    /// Page IDs where table parts were drawn
    pub page_ids: Vec<ObjectId>,
    /// Total number of pages used
    pub total_pages: usize,
    /// Final position after drawing (x, y on last page)
    pub final_position: (f32, f32),
}

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

    /// Draw a table with automatic page wrapping
    ///
    /// This method will automatically create new pages as needed when the table
    /// exceeds the available space on the current page. Header rows will be
    /// repeated on each new page if configured.
    ///
    /// # Arguments
    /// * `page_id` - The object ID of the starting page
    /// * `table` - The table to draw
    /// * `position` - The (x, y) position of the table's top-left corner
    ///
    /// # Returns
    /// Returns a PagedTableResult with information about pages used
    fn draw_table_with_pagination(
        &mut self,
        page_id: ObjectId,
        table: Table,
        position: (f32, f32),
    ) -> Result<PagedTableResult>;
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
        let position = (DEFAULT_MARGIN, A4_HEIGHT - DEFAULT_MARGIN - 50.0);
        self.draw_table(page_id, table, position)
    }

    fn create_table_content(&self, table: &Table, position: (f32, f32)) -> Result<Vec<Object>> {
        let layout = layout::calculate_layout(table)?;
        drawing::generate_table_operations(table, &layout, position)
    }

    #[instrument(skip(self, table), fields(table_rows = table.rows.len()))]
    fn draw_table_with_pagination(
        &mut self,
        page_id: ObjectId,
        table: Table,
        position: (f32, f32),
    ) -> Result<PagedTableResult> {
        debug!("Drawing paginated table at position {:?}", position);

        // Calculate layout
        let layout = layout::calculate_layout(&table)?;
        trace!("Calculated layout: {:?}", layout);

        // Generate paginated drawing operations
        let result = drawing::draw_table_paginated(self, page_id, &table, &layout, position)?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Document, Object, dictionary};

    #[test]
    fn test_basic_table_creation() {
        let table = Table::new()
            .add_row(Row::new(vec![Cell::new("Header 1"), Cell::new("Header 2")]))
            .add_row(Row::new(vec![Cell::new("Data 1"), Cell::new("Data 2")]));

        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].cells.len(), 2);
    }

    #[test]
    fn test_backward_compat_no_metrics() {
        // Tables without font_metrics should still work identically
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        if let Ok(Object::Dictionary(pages)) = doc.get_object_mut(pages_id) {
            if let Ok(Object::Array(kids)) = pages.get_mut(b"Kids") {
                kids.push(page_id.into());
            }
            pages.set("Count", Object::Integer(1));
        }
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
            page.set("Resources", resources_id);
        }
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let table = Table::new()
            .add_row(Row::new(vec![Cell::new("A"), Cell::new("B")]))
            .add_row(Row::new(vec![Cell::new("C"), Cell::new("D")]))
            .with_border(1.0);

        assert!(table.font_metrics.is_none());
        let result = doc.draw_table(page_id, table, (50.0, 750.0));
        assert!(result.is_ok());
    }

    #[test]
    fn test_unicode_table_no_metrics() {
        // Unicode text in a table without metrics should not panic
        let table = Table::new()
            .add_row(Row::new(vec![
                Cell::new("caf\u{00e9}"),
                Cell::new("\u{00fc}ber"),
            ]))
            .add_row(Row::new(vec![
                Cell::new("\u{4f60}\u{597d}"),
                Cell::new("\u{00a9} 2025"),
            ]));

        assert_eq!(table.rows.len(), 2);
        let layout = layout::calculate_layout(&table);
        assert!(layout.is_ok());
    }
}
