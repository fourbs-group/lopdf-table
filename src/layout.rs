//! Layout calculation for tables

use crate::Result;
use crate::constants::*;
use crate::error::TableError;
use crate::table::{ColumnWidth, Table};
use tracing::{debug, trace};

/// Calculated layout information for a table
#[derive(Debug, Clone)]
pub struct TableLayout {
    pub column_widths: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub total_width: f32,
    pub total_height: f32,
}

fn cell_is_bold(cell: &crate::table::Cell) -> bool {
    cell.style.as_ref().map(|s| s.bold).unwrap_or(false)
}

fn metrics_for_cell<'a>(
    table: &'a Table,
    cell: &crate::table::Cell,
) -> Option<&'a dyn crate::font::FontMetrics> {
    if cell_is_bold(cell) {
        table
            .bold_font_metrics
            .as_ref()
            .map(|m| m.as_ref())
            .or(table.font_metrics.as_ref().map(|m| m.as_ref()))
    } else {
        table.font_metrics.as_ref().map(|m| m.as_ref())
    }
}

/// Calculate the layout for a table
pub fn calculate_layout(table: &Table) -> Result<TableLayout> {
    table.validate()?;

    debug!(
        "Calculating layout for table with {} rows",
        table.rows.len()
    );

    // Determine the available table width
    let available_width = table.total_width.unwrap_or_else(|| {
        // If no total width specified, calculate based on content
        estimate_total_width(table)
    });

    // Calculate column widths based on specifications
    let column_widths = if let Some(ref width_specs) = table.column_widths {
        resolve_column_widths(width_specs, available_width, table)?
    } else {
        calculate_column_widths(table)?
    };

    // Calculate row heights (considering text wrapping if enabled)
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

/// Estimate total table width based on content
fn estimate_total_width(_table: &Table) -> f32 {
    // Default to a reasonable page width minus margins
    // Standard US Letter is 612 points wide, leave 50 points margin on each side
    LETTER_WIDTH - (DEFAULT_MARGIN * 2.0)
}

/// Resolve column widths from specifications
fn resolve_column_widths(
    specs: &[ColumnWidth],
    available_width: f32,
    table: &Table,
) -> Result<Vec<f32>> {
    let mut resolved_widths = vec![0.0; specs.len()];
    let mut total_fixed_width = 0.0;
    let mut total_percentage = 0.0;
    let mut auto_columns = Vec::new();

    // First pass: calculate fixed widths and percentages
    for (i, spec) in specs.iter().enumerate() {
        match spec {
            ColumnWidth::Pixels(width) => {
                resolved_widths[i] = *width;
                total_fixed_width += width;
            }
            ColumnWidth::Percentage(percent) => {
                total_percentage += percent;
            }
            ColumnWidth::Auto => {
                auto_columns.push(i);
            }
        }
    }

    // Calculate remaining width for auto columns
    let percentage_width = available_width * (total_percentage / 100.0);
    let remaining_width = available_width - total_fixed_width - percentage_width;

    // Second pass: resolve percentage widths
    for (i, spec) in specs.iter().enumerate() {
        if let ColumnWidth::Percentage(percent) = spec {
            resolved_widths[i] = available_width * (percent / 100.0);
        }
    }

    // Third pass: distribute remaining width among auto columns
    if !auto_columns.is_empty() {
        if remaining_width > 0.0 {
            // Calculate content-based proportions for auto columns
            let mut auto_proportions = vec![0.0; auto_columns.len()];
            let mut total_proportion = 0.0;

            for (idx, &col) in auto_columns.iter().enumerate() {
                // Estimate width based on content
                let max_content_width = estimate_column_content_width(table, col);
                auto_proportions[idx] = max_content_width;
                total_proportion += max_content_width;
            }

            // Distribute remaining width proportionally
            for (idx, &col) in auto_columns.iter().enumerate() {
                if total_proportion > 0.0 {
                    resolved_widths[col] =
                        remaining_width * (auto_proportions[idx] / total_proportion);
                } else {
                    resolved_widths[col] = remaining_width / auto_columns.len() as f32;
                }
                // Ensure minimum width
                resolved_widths[col] = resolved_widths[col].max(MIN_COLUMN_WIDTH);
            }
        } else {
            // If no remaining width, give auto columns a minimum width
            for &col in &auto_columns {
                resolved_widths[col] = MIN_COLUMN_WIDTH;
            }
        }
    }

    trace!("Resolved column widths: {:?}", resolved_widths);
    Ok(resolved_widths)
}

/// Estimate content width for a specific column
fn estimate_column_content_width(table: &Table, col_idx: usize) -> f32 {
    let mut max_width = 0.0;

    for row in &table.rows {
        if col_idx < row.cells.len() {
            let cell = &row.cells[col_idx];
            let font_size = cell
                .style
                .as_ref()
                .and_then(|s| s.font_size)
                .unwrap_or(table.style.default_font_size);

            let estimated_width = if let Some(metrics) = metrics_for_cell(table, cell) {
                crate::drawing_utils::estimate_text_width_with_metrics(
                    &cell.content,
                    font_size,
                    metrics,
                )
            } else {
                crate::drawing_utils::estimate_text_width(&cell.content, font_size)
            };
            max_width = f32::max(max_width, estimated_width);
        }
    }

    // Add padding
    let padding = table.style.padding.left + table.style.padding.right;
    max_width + padding
}

/// Calculate automatic column widths based on content
fn calculate_column_widths(table: &Table) -> Result<Vec<f32>> {
    let col_count = table.column_count();
    if col_count == 0 {
        return Err(TableError::LayoutError("No columns in table".to_string()));
    }

    let mut max_widths = vec![0.0; col_count];

    for row in &table.rows {
        for (i, cell) in row.cells.iter().enumerate() {
            if i >= col_count {
                break;
            }

            let font_size = cell
                .style
                .as_ref()
                .and_then(|s| s.font_size)
                .unwrap_or(table.style.default_font_size);

            let estimated_width = if let Some(metrics) = metrics_for_cell(table, cell) {
                crate::drawing_utils::estimate_text_width_with_metrics(
                    &cell.content,
                    font_size,
                    metrics,
                )
            } else {
                crate::drawing_utils::estimate_text_width(&cell.content, font_size)
            };

            max_widths[i] = f32::max(max_widths[i], estimated_width);
        }
    }

    // Add padding
    let padding = table.style.padding.left + table.style.padding.right;
    for width in &mut max_widths {
        *width += padding;
        // Ensure minimum width
        *width = width.max(MIN_COLUMN_WIDTH);
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

                // Calculate available width for text
                let available_width =
                    column_widths[i] - table.style.padding.left - table.style.padding.right;

                // Calculate height based on whether text wrapping is enabled
                let estimated_height = if cell.text_wrap {
                    if let Some(metrics) = metrics_for_cell(table, cell) {
                        crate::text::calculate_wrapped_text_height_with_metrics(
                            &cell.content,
                            available_width,
                            font_size,
                            DEFAULT_LINE_HEIGHT_MULTIPLIER,
                            metrics,
                        )
                    } else {
                        crate::text::calculate_wrapped_text_height(
                            &cell.content,
                            available_width,
                            font_size,
                            DEFAULT_LINE_HEIGHT_MULTIPLIER,
                        )
                    }
                } else {
                    // Single line height
                    font_size_to_height(font_size)
                };

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

/// Convert font size to line height
fn font_size_to_height(font_size: f32) -> f32 {
    // Standard line height is typically 1.2x font size
    font_size * DEFAULT_LINE_HEIGHT_MULTIPLIER
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
