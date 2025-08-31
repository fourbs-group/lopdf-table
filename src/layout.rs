//! Layout calculation for tables

use crate::Result;
use crate::error::TableError;
use crate::table::Table;
use tracing::{debug, trace};

/// Calculated layout information for a table
#[derive(Debug, Clone)]
pub struct TableLayout {
    pub column_widths: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub total_width: f32,
    pub total_height: f32,
}

/// Calculate the layout for a table
pub fn calculate_layout(table: &Table) -> Result<TableLayout> {
    table.validate()?;

    debug!(
        "Calculating layout for table with {} rows",
        table.rows.len()
    );

    // Calculate column widths
    let column_widths = if let Some(ref widths) = table.column_widths {
        widths.clone()
    } else {
        calculate_column_widths(table)?
    };

    // Calculate row heights
    let row_heights = calculate_row_heights(table, &column_widths)?;

    // Calculate totals
    let total_width = column_widths.iter().sum();
    let total_height = row_heights.iter().sum();

    trace!("Layout calculated: {}x{}", total_width, total_height);

    Ok(TableLayout {
        column_widths,
        row_heights,
        total_width,
        total_height,
    })
}

/// Calculate automatic column widths based on content
fn calculate_column_widths(table: &Table) -> Result<Vec<f32>> {
    let col_count = table.column_count();
    if col_count == 0 {
        return Err(TableError::LayoutError("No columns in table".to_string()));
    }

    // For now, use a simple heuristic based on max content length
    let mut max_widths = vec![0.0; col_count];

    for row in &table.rows {
        for (i, cell) in row.cells.iter().enumerate() {
            if i >= col_count {
                break;
            }

            // Estimate width based on character count
            // This is a simplified calculation - real implementation would measure text
            let estimated_width = estimate_text_width(
                &cell.content,
                cell.style
                    .as_ref()
                    .and_then(|s| s.font_size)
                    .unwrap_or(table.style.default_font_size),
            );

            max_widths[i] = f32::max(max_widths[i], estimated_width);
        }
    }

    // Add padding
    let padding = table.style.padding.left + table.style.padding.right;
    for width in &mut max_widths {
        *width += padding;
        // Ensure minimum width
        *width = width.max(20.0);
    }

    trace!("Calculated column widths: {:?}", max_widths);
    Ok(max_widths)
}

/// Calculate row heights based on content
fn calculate_row_heights(table: &Table, column_widths: &[f32]) -> Result<Vec<f32>> {
    let mut heights = Vec::with_capacity(table.rows.len());

    for row in &table.rows {
        if let Some(height) = row.height {
            heights.push(height);
        } else {
            // Calculate based on content
            let mut max_height = 0.0;

            for (i, cell) in row.cells.iter().enumerate() {
                if i >= column_widths.len() {
                    break;
                }

                let font_size = cell
                    .style
                    .as_ref()
                    .and_then(|s| s.font_size)
                    .unwrap_or(table.style.default_font_size);

                // Estimate height based on text wrapping
                let available_width =
                    column_widths[i] - table.style.padding.left - table.style.padding.right;

                let estimated_height =
                    estimate_text_height(&cell.content, available_width, font_size);

                max_height = f32::max(max_height, estimated_height);
            }

            // Add padding
            max_height += table.style.padding.top + table.style.padding.bottom;
            // Ensure minimum height
            max_height = max_height.max(font_size_to_height(table.style.default_font_size));

            heights.push(max_height);
        }
    }

    trace!("Calculated row heights: {:?}", heights);
    Ok(heights)
}

/// Estimate text width based on character count and font size
fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    // Simplified estimation: average character width is ~0.5 of font size
    let char_count = text.chars().count() as f32;
    char_count * font_size * 0.5
}

/// Estimate text height based on wrapping
fn estimate_text_height(text: &str, available_width: f32, font_size: f32) -> f32 {
    if text.is_empty() {
        return font_size_to_height(font_size);
    }

    let text_width = estimate_text_width(text, font_size);
    let lines = (text_width / available_width).ceil().max(1.0);

    lines * font_size_to_height(font_size)
}

/// Convert font size to line height
fn font_size_to_height(font_size: f32) -> f32 {
    // Standard line height is typically 1.2x font size
    font_size * 1.2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{Cell, Row};

    #[test]
    fn test_layout_calculation() {
        let table = Table::new()
            .add_row(Row::new(vec![
                Cell::new("Short"),
                Cell::new("Medium text"),
                Cell::new("This is a longer piece of text"),
            ]))
            .add_row(Row::new(vec![
                Cell::new("A"),
                Cell::new("B"),
                Cell::new("C"),
            ]));

        let layout = calculate_layout(&table).unwrap();

        assert_eq!(layout.column_widths.len(), 3);
        assert_eq!(layout.row_heights.len(), 2);
        assert!(layout.total_width > 0.0);
        assert!(layout.total_height > 0.0);
    }
}
