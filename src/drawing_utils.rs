//! Shared drawing utilities for PDF table operations

use crate::constants::*;
use crate::layout::TableLayout;
use crate::style::{BorderStyle, Color};
use crate::table::Table;
use lopdf::{Object, content::Operation};

/// Check if a string represents a PDF operator
pub fn is_pdf_operator(name: &str) -> bool {
    match name {
        // Text operators
        "BT" | "ET" | "Tf" | "Td" | "Tj" | "TJ" | "Tm" => true,
        // Marked-content operators
        "BMC" | "BDC" | "EMC" | "MP" | "DP" | "BX" | "EX" => true,
        // Color operators
        "rg" | "RG" | "g" | "G" => true,
        // Graphics state save/restore, clipping, and ExtGState
        "q" | "Q" | "W" | "W*" | "gs" => true,
        // XObject and transformation operators
        "Do" | "cm" => true,
        // Path construction
        "m" | "l" | "c" | "v" | "y" | "h" | "re" => true,
        // Path painting
        "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => true,
        // Line width
        "w" => true,
        // Other operators that start with lowercase
        _ if name.chars().next().map_or(false, |c| c.is_lowercase()) => true,
        _ => false,
    }
}

/// Draw a filled rectangle
pub fn draw_rectangle_fill(x: f32, y: f32, width: f32, height: f32, color: Color) -> Vec<Object> {
    vec![
        // Set fill color
        Object::Name(b"rg".to_vec()),
        color.r.into(),
        color.g.into(),
        color.b.into(),
        // Draw rectangle
        Object::Name(b"re".to_vec()),
        x.into(),
        y.into(),
        width.into(),
        height.into(),
        // Fill
        Object::Name(b"f".to_vec()),
    ]
}

/// Set stroke color and width for drawing operations
pub fn set_stroke_style(color: Color, width: f32) -> Vec<Object> {
    vec![
        Object::Name(b"RG".to_vec()),
        color.r.into(),
        color.g.into(),
        color.b.into(),
        Object::Name(b"w".to_vec()),
        width.into(),
    ]
}

/// Draw a stroked rectangle (outline only)
pub fn draw_rectangle_stroke(x: f32, y: f32, width: f32, height: f32) -> Vec<Object> {
    vec![
        Object::Name(b"re".to_vec()),
        x.into(),
        y.into(),
        width.into(),
        height.into(),
        Object::Name(b"S".to_vec()),
    ]
}

/// Draw a horizontal line
pub fn draw_horizontal_line(start_x: f32, end_x: f32, y: f32) -> Vec<Object> {
    vec![
        Object::Name(b"m".to_vec()),
        start_x.into(),
        y.into(),
        Object::Name(b"l".to_vec()),
        end_x.into(),
        y.into(),
        Object::Name(b"S".to_vec()),
    ]
}

/// Draw a vertical line
pub fn draw_vertical_line(x: f32, start_y: f32, end_y: f32) -> Vec<Object> {
    vec![
        Object::Name(b"m".to_vec()),
        x.into(),
        start_y.into(),
        Object::Name(b"l".to_vec()),
        x.into(),
        end_y.into(),
        Object::Name(b"S".to_vec()),
    ]
}

/// Calculate the total width for a cell with colspan
pub fn calculate_cell_width(col_idx: usize, colspan: usize, column_widths: &[f32]) -> f32 {
    if colspan > 1 {
        let end_col = (col_idx + colspan).min(column_widths.len());
        column_widths[col_idx..end_col].iter().sum()
    } else {
        column_widths.get(col_idx).copied().unwrap_or(0.0)
    }
}

/// Estimate text width based on character count and font size
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let char_count = text.chars().count() as f32;
    char_count * font_size * DEFAULT_CHAR_WIDTH_RATIO
}

/// Measure text width using font metrics
pub fn estimate_text_width_with_metrics(
    text: &str,
    font_size: f32,
    metrics: &dyn crate::font::FontMetrics,
) -> f32 {
    metrics.text_width(text, font_size)
}

/// Convert a vector of Objects to Content operations
pub fn objects_to_operations(objects: &[Object]) -> Vec<Operation> {
    let mut operations = Vec::new();
    let mut i = 0;

    while i < objects.len() {
        if let Object::Name(ref name) = objects[i] {
            let name_str = String::from_utf8_lossy(name);

            if is_pdf_operator(&name_str) {
                let operator = name_str.to_string();
                let mut operands = Vec::new();

                // Collect operands until next operator
                i += 1;
                while i < objects.len() {
                    if let Object::Name(ref next_name) = objects[i] {
                        let next_str = String::from_utf8_lossy(next_name);
                        if is_pdf_operator(&next_str) {
                            break;
                        }
                    }
                    operands.push(objects[i].clone());
                    i += 1;
                }

                operations.push(Operation::new(&operator, operands));
            } else {
                // This Name is an operand, not an operator
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    operations
}

/// Enum to specify border drawing mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderDrawingMode {
    /// Draw borders for the entire table
    Full,
    /// Draw borders for a subset of rows
    Subset(f32), // subset_height
}

/// Draw table borders (handles both full and subset modes)
pub fn draw_table_borders(
    table: &Table,
    layout: &TableLayout,
    position: (f32, f32),
    mode: BorderDrawingMode,
    row_indices: Option<&[usize]>,
) -> Vec<Object> {
    let mut operations = Vec::new();
    let (start_x, start_y) = position;

    if table.style.border_style == BorderStyle::None {
        return operations;
    }

    // Set stroke color and width
    operations.extend(set_stroke_style(
        table.style.border_color,
        table.style.border_width,
    ));

    // Determine height based on mode
    let total_height = match mode {
        BorderDrawingMode::Full => layout.total_height,
        BorderDrawingMode::Subset(height) => height,
    };

    // Draw outer border
    operations.extend(draw_rectangle_stroke(
        start_x,
        start_y - total_height,
        layout.total_width,
        total_height,
    ));

    // Draw horizontal lines between rows
    let rows_to_process: Vec<usize> = match row_indices {
        Some(indices) => indices.to_vec(),
        None => (0..layout.row_heights.len()).collect(),
    };

    let mut current_y = start_y;
    for (idx, &row_idx) in rows_to_process.iter().enumerate() {
        if idx > 0 {
            operations.extend(draw_horizontal_line(
                start_x,
                start_x + layout.total_width,
                current_y,
            ));
        }
        if row_idx < layout.row_heights.len() {
            current_y -= layout.row_heights[row_idx];
        }
    }

    // Draw vertical lines between columns (handling colspan)
    for (idx, &row_idx) in rows_to_process.iter().enumerate() {
        if row_idx >= table.rows.len() {
            continue;
        }

        let row = &table.rows[row_idx];
        let mut current_x = start_x;
        let mut col_idx = 0;

        let row_y_top = if let Some(indices) = row_indices {
            start_y
                - indices
                    .iter()
                    .take(idx)
                    .map(|&i| layout.row_heights[i])
                    .sum::<f32>()
        } else {
            start_y - layout.row_heights.iter().take(row_idx).sum::<f32>()
        };
        let row_y_bottom = row_y_top - layout.row_heights[row_idx];

        for cell in &row.cells {
            if col_idx >= layout.column_widths.len() {
                break;
            }

            // Draw vertical line at the start of this cell (if not first column)
            if col_idx > 0 {
                operations.extend(draw_vertical_line(current_x, row_y_top, row_y_bottom));
            }

            // Move across the span of this cell
            let cell_span = cell.colspan.max(1);
            for span_idx in 0..cell_span {
                if col_idx + span_idx < layout.column_widths.len() {
                    current_x += layout.column_widths[col_idx + span_idx];
                }
            }
            col_idx += cell_span;
        }
    }

    operations
}
