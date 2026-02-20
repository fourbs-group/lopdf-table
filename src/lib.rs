//! A composable table drawing library for PDFs built on lopdf
//!
//! This library provides an ergonomic API for creating tables in PDF documents
//! with support for automatic sizing, custom styling, and flexible layouts.

use lopdf::content::Operation;
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

/// Optional hook for injecting tagged content around table cells.
pub trait TaggedCellHook {
    fn begin_cell(&mut self, row: usize, col: usize, is_header: bool) -> Vec<Operation>;
    fn end_cell(&mut self, row: usize, col: usize, is_header: bool) -> Vec<Operation>;
}

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

    /// Draw a table with an optional tagged-cell hook.
    ///
    /// Existing rendering behavior is unchanged when `hook` is `None`.
    fn draw_table_with_hook(
        &mut self,
        page_id: ObjectId,
        table: Table,
        position: (f32, f32),
        hook: Option<&mut dyn TaggedCellHook>,
    ) -> Result<()>;

    /// Draw a paginated table with an optional tagged-cell hook.
    ///
    /// Existing rendering behavior is unchanged when `hook` is `None`.
    fn draw_table_with_pagination_and_hook(
        &mut self,
        page_id: ObjectId,
        table: Table,
        position: (f32, f32),
        hook: Option<&mut dyn TaggedCellHook>,
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
        let operations = drawing::generate_table_operations(&table, &layout, position, None)?;

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
        drawing::generate_table_operations(table, &layout, position, None)
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
        let result = drawing::draw_table_paginated(self, page_id, &table, &layout, position, None)?;

        Ok(result)
    }

    fn draw_table_with_hook(
        &mut self,
        page_id: ObjectId,
        table: Table,
        position: (f32, f32),
        hook: Option<&mut dyn TaggedCellHook>,
    ) -> Result<()> {
        debug!("Drawing table with hook at position {:?}", position);
        let layout = layout::calculate_layout(&table)?;
        let operations = drawing::generate_table_operations(&table, &layout, position, hook)?;
        drawing::add_operations_to_page(self, page_id, operations)?;
        Ok(())
    }

    fn draw_table_with_pagination_and_hook(
        &mut self,
        page_id: ObjectId,
        table: Table,
        position: (f32, f32),
        hook: Option<&mut dyn TaggedCellHook>,
    ) -> Result<PagedTableResult> {
        debug!(
            "Drawing paginated table with hook at position {:?}",
            position
        );
        let layout = layout::calculate_layout(&table)?;
        drawing::draw_table_paginated(self, page_id, &table, &layout, position, hook)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
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

    #[derive(Clone)]
    struct MockMetrics {
        char_width_pts: f32,
    }

    impl FontMetrics for MockMetrics {
        fn char_width(&self, _ch: char, _font_size: f32) -> f32 {
            self.char_width_pts
        }

        fn text_width(&self, text: &str, _font_size: f32) -> f32 {
            text.chars().count() as f32 * self.char_width_pts
        }

        fn encode_text(&self, text: &str) -> Vec<u8> {
            vec![0; text.chars().count() * 2]
        }
    }

    fn extract_tf_font_names(objects: &[Object]) -> Vec<String> {
        let mut names = Vec::new();
        let mut i = 0usize;
        while i + 1 < objects.len() {
            if let Object::Name(op) = &objects[i] {
                if op.as_slice() == b"Tf" {
                    if let Object::Name(font_name) = &objects[i + 1] {
                        names.push(String::from_utf8_lossy(font_name).to_string());
                    }
                }
            }
            i += 1;
        }
        names
    }

    #[derive(Debug, Clone, Copy)]
    struct RectExtents {
        max_top: f32,
        min_bottom: f32,
    }

    fn object_to_f32(object: &Object) -> Option<f32> {
        match object {
            Object::Integer(v) => Some(*v as f32),
            Object::Real(v) => Some(*v),
            _ => None,
        }
    }

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() <= 0.001
    }

    fn op_has_rgb(op: &Operation, operator: &str, color: Color) -> bool {
        op.operator == operator
            && op.operands.len() == 3
            && object_to_f32(&op.operands[0]).map_or(false, |v| approx_eq(v, color.r))
            && object_to_f32(&op.operands[1]).map_or(false, |v| approx_eq(v, color.g))
            && object_to_f32(&op.operands[2]).map_or(false, |v| approx_eq(v, color.b))
    }

    fn op_has_line_width(op: &Operation, width: f32) -> bool {
        op.operator == "w"
            && op.operands.len() == 1
            && object_to_f32(&op.operands[0]).map_or(false, |v| approx_eq(v, width))
    }

    fn has_stroke_style(operations: &[Operation], color: Color, width: f32) -> bool {
        operations.iter().any(|op| op_has_rgb(op, "RG", color))
            && operations.iter().any(|op| op_has_line_width(op, width))
    }

    fn page_content_operations(doc: &Document, page_id: ObjectId) -> Vec<Operation> {
        let bytes = doc
            .get_page_content(page_id)
            .expect("page content should be readable");
        Content::decode(&bytes)
            .expect("page content should decode")
            .operations
    }

    fn page_rect_extents(doc: &Document, page_id: ObjectId) -> Option<RectExtents> {
        let bytes = doc.get_page_content(page_id).ok()?;
        let content = Content::decode(&bytes).ok()?;
        let mut max_top = f32::NEG_INFINITY;
        let mut min_bottom = f32::INFINITY;
        let mut found = false;

        for op in content.operations {
            if op.operator != "re" || op.operands.len() != 4 {
                continue;
            }
            let y = object_to_f32(&op.operands[1])?;
            let h = object_to_f32(&op.operands[3])?;
            max_top = max_top.max(y + h);
            min_bottom = min_bottom.min(y);
            found = true;
        }

        if found {
            Some(RectExtents {
                max_top,
                min_bottom,
            })
        } else {
            None
        }
    }

    #[test]
    fn test_cell_border_overrides_emit_custom_stroke_ops() {
        let custom_color = Color::rgb(0.11, 0.22, 0.33);
        let custom_width = 2.75;
        let header_style = CellStyle {
            border_top: Some((BorderStyle::Solid, custom_width, custom_color)),
            ..Default::default()
        };

        let table = Table::new()
            .with_pixel_widths(vec![180.0])
            .add_row(Row::new(vec![Cell::new("Header").with_style(header_style)]));

        let objects = Document::with_version("1.7")
            .create_table_content(&table, (50.0, 750.0))
            .expect("table content should be generated");
        let operations = crate::drawing_utils::objects_to_operations(&objects);

        assert!(
            has_stroke_style(&operations, custom_color, custom_width),
            "expected custom border stroke ops (color + width) to be emitted"
        );
    }

    #[test]
    fn test_cell_background_fill_ops_still_emitted_with_styled_cells() {
        let bg_color = Color::rgb(0.13, 0.27, 0.71);
        let style = CellStyle {
            background_color: Some(bg_color),
            ..Default::default()
        };

        let table = Table::new()
            .with_pixel_widths(vec![180.0])
            .add_row(Row::new(vec![Cell::new("Header").with_style(style)]));

        let objects = Document::with_version("1.7")
            .create_table_content(&table, (50.0, 750.0))
            .expect("table content should be generated");
        let operations = crate::drawing_utils::objects_to_operations(&objects);

        let has_bg_color = operations.iter().any(|op| op_has_rgb(op, "rg", bg_color));
        let has_fill = operations.iter().any(|op| op.operator == "f");

        assert!(
            has_bg_color && has_fill,
            "expected background color fill ops to be present for styled cells"
        );
    }

    #[test]
    fn test_embedded_bold_resource_selected_for_bold_cells() {
        let mut style = TableStyle::default();
        style.embedded_font_resource_name = Some("EF0".to_string());
        style.embedded_font_resource_name_bold = Some("EF0B".to_string());

        let table = Table::new()
            .with_style(style)
            .add_row(Row::new(vec![Cell::new("Header").bold()]))
            .with_font_metrics(MockMetrics {
                char_width_pts: 5.0,
            })
            .with_bold_font_metrics(MockMetrics {
                char_width_pts: 9.0,
            });

        let ops = Document::with_version("1.5")
            .create_table_content(&table, (50.0, 750.0))
            .expect("table content should be generated");
        let font_names = extract_tf_font_names(&ops);
        assert!(
            font_names.iter().any(|name| name == "EF0B"),
            "expected bold embedded font resource EF0B, got: {:?}",
            font_names
        );
    }

    #[test]
    fn test_embedded_regular_resource_used_as_bold_fallback() {
        let mut style = TableStyle::default();
        style.embedded_font_resource_name = Some("EF0".to_string());
        style.embedded_font_resource_name_bold = None;

        let table = Table::new()
            .with_style(style)
            .add_row(Row::new(vec![Cell::new("Header").bold()]))
            .with_font_metrics(MockMetrics {
                char_width_pts: 5.0,
            });

        let ops = Document::with_version("1.5")
            .create_table_content(&table, (50.0, 750.0))
            .expect("table content should be generated");
        let font_names = extract_tf_font_names(&ops);
        assert!(
            font_names.iter().any(|name| name == "EF0"),
            "expected embedded font fallback EF0, got: {:?}",
            font_names
        );
    }

    #[test]
    fn test_layout_uses_bold_metrics_when_available() {
        let bold_cell = Cell::new("WWWWWW").bold();

        let table_regular_only = Table::new()
            .add_row(Row::new(vec![bold_cell.clone()]))
            .with_font_metrics(MockMetrics {
                char_width_pts: 2.0,
            });

        let table_with_bold_metrics = Table::new()
            .add_row(Row::new(vec![bold_cell]))
            .with_font_metrics(MockMetrics {
                char_width_pts: 2.0,
            })
            .with_bold_font_metrics(MockMetrics {
                char_width_pts: 8.0,
            });

        let regular_layout = layout::calculate_layout(&table_regular_only)
            .expect("layout should succeed with regular metrics only");
        let bold_layout = layout::calculate_layout(&table_with_bold_metrics)
            .expect("layout should succeed with bold metrics");

        assert!(
            bold_layout.total_width > regular_layout.total_width,
            "expected bold metrics to increase width: regular={} bold={}",
            regular_layout.total_width,
            bold_layout.total_width
        );
    }

    #[test]
    fn test_tagged_cell_hook_is_invoked() {
        struct Hook {
            begin_calls: usize,
            end_calls: usize,
        }

        impl TaggedCellHook for Hook {
            fn begin_cell(&mut self, _row: usize, _col: usize, _is_header: bool) -> Vec<Operation> {
                self.begin_calls += 1;
                vec![]
            }

            fn end_cell(&mut self, _row: usize, _col: usize, _is_header: bool) -> Vec<Operation> {
                self.end_calls += 1;
                vec![]
            }
        }

        let mut doc = Document::with_version("1.7");
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
            .add_row(Row::new(vec![Cell::new("H1"), Cell::new("H2")]))
            .add_row(Row::new(vec![Cell::new("A1"), Cell::new("A2")]))
            .with_header_rows(1);

        let mut hook = Hook {
            begin_calls: 0,
            end_calls: 0,
        };
        doc.draw_table_with_hook(page_id, table, (50.0, 750.0), Some(&mut hook))
            .expect("table draw with hook should succeed");

        assert_eq!(hook.begin_calls, 4);
        assert_eq!(hook.end_calls, 4);
    }

    #[test]
    fn test_marked_content_tokens_parse_as_operators() {
        let objects = vec![
            Object::Name(b"BDC".to_vec()),
            Object::Name(b"TH".to_vec()),
            Object::Dictionary(dictionary! { "MCID" => 0 }),
            Object::Name(b"BT".to_vec()),
            Object::Name(b"ET".to_vec()),
            Object::Name(b"EMC".to_vec()),
        ];

        let operations = crate::drawing_utils::objects_to_operations(&objects);
        assert_eq!(operations.len(), 4);
        assert_eq!(operations[0].operator, "BDC");
        assert_eq!(operations[0].operands.len(), 2);
        assert_eq!(operations[1].operator, "BT");
        assert_eq!(operations[2].operator, "ET");
        assert_eq!(operations[3].operator, "EMC");
    }

    #[test]
    fn test_hook_generated_bdc_emc_appear_in_page_content() {
        struct MarkedHook;

        impl TaggedCellHook for MarkedHook {
            fn begin_cell(&mut self, _row: usize, _col: usize, is_header: bool) -> Vec<Operation> {
                vec![Operation::new(
                    "BDC",
                    vec![
                        Object::Name(if is_header {
                            b"TH".to_vec()
                        } else {
                            b"TD".to_vec()
                        }),
                        Object::Dictionary(dictionary! { "MCID" => 0 }),
                    ],
                )]
            }

            fn end_cell(&mut self, _row: usize, _col: usize, _is_header: bool) -> Vec<Operation> {
                vec![Operation::new("EMC", vec![])]
            }
        }

        let mut doc = Document::with_version("1.7");
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
            .add_row(Row::new(vec![Cell::new("H1"), Cell::new("H2")]))
            .add_row(Row::new(vec![Cell::new("A1"), Cell::new("A2")]))
            .with_header_rows(1);

        let mut hook = MarkedHook;
        doc.draw_table_with_hook(page_id, table, (50.0, 750.0), Some(&mut hook))
            .expect("table draw with hook should succeed");

        let bytes = doc
            .get_page_content(page_id)
            .expect("page content should be readable");
        let decoded = Content::decode(&bytes).expect("content should decode");

        let bdc_count = decoded
            .operations
            .iter()
            .filter(|op| op.operator == "BDC")
            .count();
        let emc_count = decoded
            .operations
            .iter()
            .filter(|op| op.operator == "EMC")
            .count();

        assert!(bdc_count >= 4);
        assert_eq!(bdc_count, emc_count);
    }

    #[test]
    fn test_hook_mode_wraps_non_semantic_ops_as_artifact() {
        struct NoopHook;

        impl TaggedCellHook for NoopHook {
            fn begin_cell(&mut self, _row: usize, _col: usize, _is_header: bool) -> Vec<Operation> {
                vec![]
            }

            fn end_cell(&mut self, _row: usize, _col: usize, _is_header: bool) -> Vec<Operation> {
                vec![]
            }
        }

        let mut doc = Document::with_version("1.7");
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
            .with_border(0.5)
            .add_row(Row::new(vec![Cell::new("H1"), Cell::new("H2")]))
            .add_row(Row::new(vec![Cell::new("A1"), Cell::new("A2")]))
            .with_header_rows(1);

        let mut hook = NoopHook;
        doc.draw_table_with_hook(page_id, table, (50.0, 750.0), Some(&mut hook))
            .expect("table draw with hook should succeed");

        let bytes = doc
            .get_page_content(page_id)
            .expect("page content should be readable");
        let decoded = Content::decode(&bytes).expect("content should decode");

        let has_artifact_bdc = decoded.operations.iter().any(|op| {
            op.operator == "BDC"
                && op
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    == Some(b"Artifact".as_slice())
        });

        assert!(
            has_artifact_bdc,
            "expected non-semantic table drawing ops to be wrapped as Artifact"
        );
    }

    #[test]
    fn test_paginated_table_continuation_pages_use_top_margin_anchor_with_repeated_headers() {
        const PAGE_HEIGHT: f32 = 842.0;
        const TOP_MARGIN: f32 = 50.0;
        const BOTTOM_MARGIN: f32 = 50.0;
        const START_Y: f32 = 500.0;
        const EPS: f32 = 0.01;

        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), PAGE_HEIGHT.into()],
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

        let mut style = TableStyle::default();
        style.page_height = Some(PAGE_HEIGHT);
        style.top_margin = TOP_MARGIN;
        style.bottom_margin = BOTTOM_MARGIN;
        style.repeat_headers = true;

        let mut table = Table::new()
            .with_style(style)
            .with_header_rows(1)
            .with_pixel_widths(vec![300.0])
            .add_row(Row::new(vec![Cell::new("Header")]).with_height(30.0));

        for row in 0..120 {
            table =
                table.add_row(Row::new(vec![Cell::new(format!("row-{row}"))]).with_height(30.0));
        }

        let result = doc
            .draw_table_with_pagination(page_id, table, (50.0, START_Y))
            .expect("paginated table draw should succeed");

        assert!(
            result.page_ids.len() >= 3,
            "expected at least 3 pages, got {}",
            result.page_ids.len()
        );

        let first_page_extents =
            page_rect_extents(&doc, result.page_ids[0]).expect("first page should have rectangles");
        let second_page_extents = page_rect_extents(&doc, result.page_ids[1])
            .expect("second page should have rectangles");

        assert!(
            (first_page_extents.max_top - START_Y).abs() <= EPS,
            "expected first page max top ~{START_Y}, got {}",
            first_page_extents.max_top
        );
        assert!(
            (second_page_extents.max_top - (PAGE_HEIGHT - TOP_MARGIN)).abs() <= EPS,
            "expected second page max top ~{}, got {}",
            PAGE_HEIGHT - TOP_MARGIN,
            second_page_extents.max_top
        );
        assert!(
            second_page_extents.min_bottom >= BOTTOM_MARGIN - EPS,
            "expected second page min bottom >= {}, got {}",
            BOTTOM_MARGIN - EPS,
            second_page_extents.min_bottom
        );
    }

    #[test]
    fn test_paginated_repeated_header_border_overrides_render_on_continuation_pages() {
        const PAGE_HEIGHT: f32 = 842.0;
        const TOP_MARGIN: f32 = 50.0;
        const BOTTOM_MARGIN: f32 = 50.0;
        const START_Y: f32 = 500.0;
        let header_border_color = Color::rgb(0.07, 0.16, 0.29);
        let header_border_width = 2.5;

        let mut doc = Document::with_version("1.7");
        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), PAGE_HEIGHT.into()],
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

        let mut table_style = TableStyle::default();
        table_style.page_height = Some(PAGE_HEIGHT);
        table_style.top_margin = TOP_MARGIN;
        table_style.bottom_margin = BOTTOM_MARGIN;
        table_style.repeat_headers = true;

        let header_style = CellStyle {
            border_top: Some((BorderStyle::Solid, header_border_width, header_border_color)),
            border_bottom: Some((BorderStyle::Solid, header_border_width, header_border_color)),
            ..Default::default()
        };

        let mut table = Table::new()
            .with_style(table_style)
            .with_header_rows(1)
            .with_pixel_widths(vec![300.0])
            .add_row(
                Row::new(vec![Cell::new("Header").with_style(header_style)]).with_height(30.0),
            );

        for row in 0..120 {
            table =
                table.add_row(Row::new(vec![Cell::new(format!("row-{row}"))]).with_height(30.0));
        }

        let result = doc
            .draw_table_with_pagination(page_id, table, (50.0, START_Y))
            .expect("paginated table draw should succeed");

        assert!(
            result.page_ids.len() >= 2,
            "expected at least 2 pages, got {}",
            result.page_ids.len()
        );

        let second_page_ops = page_content_operations(&doc, result.page_ids[1]);
        assert!(
            has_stroke_style(&second_page_ops, header_border_color, header_border_width),
            "expected repeated header border override stroke ops on continuation page"
        );
    }
}
