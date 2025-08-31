//! PDF drawing operations for tables

use crate::Result;
use crate::layout::TableLayout;
use crate::style::{Alignment, BorderStyle, Color, VerticalAlignment};
use crate::table::Table;
use lopdf::{
    Document, Object, ObjectId,
    content::{Content, Operation},
};
use tracing::{debug, trace};

/// Generate PDF operations for drawing a table
pub fn generate_table_operations(
    table: &Table,
    layout: &TableLayout,
    position: (f32, f32),
) -> Result<Vec<Object>> {
    let mut operations = Vec::new();
    let (start_x, start_y) = position;

    debug!(
        "Generating operations for table at ({}, {})",
        start_x, start_y
    );

    // Draw table background if specified
    if let Some(bg_color) = &table.style.background_color {
        operations.extend(draw_rectangle_fill(
            start_x,
            start_y - layout.total_height,
            layout.total_width,
            layout.total_height,
            *bg_color,
        ));
    }

    // Draw cells and content
    let mut current_y = start_y;

    for (row_idx, row) in table.rows.iter().enumerate() {
        let row_height = layout.row_heights[row_idx];
        let mut current_x = start_x;

        // Draw row background if specified
        if let Some(ref row_style) = row.style {
            if let Some(bg_color) = row_style.background_color {
                operations.extend(draw_rectangle_fill(
                    start_x,
                    current_y - row_height,
                    layout.total_width,
                    row_height,
                    bg_color,
                ));
            }
        }

        for (col_idx, cell) in row.cells.iter().enumerate() {
            let col_width = layout.column_widths[col_idx];

            // Draw cell background if specified
            if let Some(ref cell_style) = cell.style {
                if let Some(bg_color) = cell_style.background_color {
                    operations.extend(draw_rectangle_fill(
                        current_x,
                        current_y - row_height,
                        col_width,
                        row_height,
                        bg_color,
                    ));
                }
            }

            // Draw cell content (text)
            operations.extend(draw_cell_text(
                cell, table, current_x, current_y, col_width, row_height,
            )?);

            current_x += col_width;
        }

        current_y -= row_height;
    }

    // Draw table borders
    operations.extend(draw_table_borders(table, layout, position));

    trace!("Generated {} operations", operations.len());
    Ok(operations)
}

/// Draw a filled rectangle
fn draw_rectangle_fill(x: f32, y: f32, width: f32, height: f32, color: Color) -> Vec<Object> {
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

/// Draw table borders
fn draw_table_borders(table: &Table, layout: &TableLayout, position: (f32, f32)) -> Vec<Object> {
    let mut operations = Vec::new();
    let (start_x, start_y) = position;

    if table.style.border_style == BorderStyle::None {
        return operations;
    }

    // Set stroke color
    operations.extend(vec![
        Object::Name(b"RG".to_vec()),
        table.style.border_color.r.into(),
        table.style.border_color.g.into(),
        table.style.border_color.b.into(),
    ]);

    // Set line width
    operations.extend(vec![
        Object::Name(b"w".to_vec()),
        table.style.border_width.into(),
    ]);

    // Draw outer border
    operations.extend(vec![
        Object::Name(b"re".to_vec()),
        start_x.into(),
        (start_y - layout.total_height).into(),
        layout.total_width.into(),
        layout.total_height.into(),
        Object::Name(b"S".to_vec()),
    ]);

    // Draw horizontal lines between rows
    let mut current_y = start_y;
    for (i, height) in layout.row_heights.iter().enumerate() {
        if i > 0 {
            operations.extend(vec![
                Object::Name(b"m".to_vec()),
                start_x.into(),
                current_y.into(),
                Object::Name(b"l".to_vec()),
                (start_x + layout.total_width).into(),
                current_y.into(),
                Object::Name(b"S".to_vec()),
            ]);
        }
        current_y -= height;
    }

    // Draw vertical lines between columns
    let mut current_x = start_x;
    for (i, width) in layout.column_widths.iter().enumerate() {
        if i > 0 {
            operations.extend(vec![
                Object::Name(b"m".to_vec()),
                current_x.into(),
                start_y.into(),
                Object::Name(b"l".to_vec()),
                current_x.into(),
                (start_y - layout.total_height).into(),
                Object::Name(b"S".to_vec()),
            ]);
        }
        current_x += width;
    }

    operations
}

/// Draw text within a cell (returns Operation objects directly)
fn draw_cell_text_operations(
    cell: &crate::table::Cell,
    table: &Table,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Vec<Operation> {
    if cell.content.is_empty() {
        return Vec::new();
    }

    let mut operations = Vec::new();

    // Get text styling
    let font_size = cell
        .style
        .as_ref()
        .and_then(|s| s.font_size)
        .unwrap_or(table.style.default_font_size);

    let text_color = cell
        .style
        .as_ref()
        .map(|s| s.text_color)
        .unwrap_or(Color::black());

    let alignment = cell
        .style
        .as_ref()
        .map(|s| s.alignment)
        .unwrap_or(Alignment::Left);

    let v_alignment = cell
        .style
        .as_ref()
        .map(|s| s.vertical_alignment)
        .unwrap_or(VerticalAlignment::Middle);

    // Calculate text position with padding
    let padding = cell
        .style
        .as_ref()
        .and_then(|s| s.padding.as_ref())
        .unwrap_or(&table.style.padding);

    // Estimate text width for alignment
    // This is a simplified estimation - real implementation would use font metrics
    let estimated_text_width = cell.content.len() as f32 * font_size * 0.5;

    let text_x = match alignment {
        Alignment::Left => x + padding.left,
        Alignment::Center => x + width / 2.0 - estimated_text_width / 2.0,
        Alignment::Right => x + width - padding.right - estimated_text_width,
    };

    // PDF text positioning uses baseline, not top of text
    // For better vertical centering, we need to account for descenders
    let text_y = match v_alignment {
        VerticalAlignment::Top => y - padding.top - font_size,
        VerticalAlignment::Middle => y - height / 2.0 - font_size * 0.35, // Slightly below center for visual balance
        VerticalAlignment::Bottom => y - height + padding.bottom + font_size * 0.2,
    };

    // Begin text object
    operations.push(Operation::new("BT", vec![]));

    // Set font
    let font_name = if cell.style.as_ref().map_or(false, |s| s.bold) {
        "F1-Bold"
    } else {
        "F1"
    };

    operations.push(Operation::new(
        "Tf",
        vec![
            Object::Name(font_name.as_bytes().to_vec()),
            font_size.into(),
        ],
    ));

    // Set text color
    operations.push(Operation::new(
        "rg",
        vec![
            text_color.r.into(),
            text_color.g.into(),
            text_color.b.into(),
        ],
    ));

    // Position text
    operations.push(Operation::new("Td", vec![text_x.into(), text_y.into()]));

    // Show text
    operations.push(Operation::new(
        "Tj",
        vec![Object::string_literal(cell.content.clone())],
    ));

    // End text object
    operations.push(Operation::new("ET", vec![]));

    operations
}

/// Draw text within a cell
fn draw_cell_text(
    cell: &crate::table::Cell,
    table: &Table,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<Vec<Object>> {
    // This now converts Operations to the flat Object list for compatibility
    let ops = draw_cell_text_operations(cell, table, x, y, width, height);
    let mut objects = Vec::new();

    for op in ops {
        objects.push(Object::Name(op.operator.as_bytes().to_vec()));
        objects.extend(op.operands);
    }

    Ok(objects)
}

/// Add operations to a page in the document
pub fn add_operations_to_page(
    doc: &mut Document,
    page_id: ObjectId,
    operations: Vec<Object>,
) -> Result<()> {
    debug!(
        "Adding {} operations to page {:?}",
        operations.len(),
        page_id
    );
    trace!("Raw operations: {:?}", operations);

    // Convert operations to Content
    let mut content_ops = Vec::new();
    let mut i = 0;

    while i < operations.len() {
        if let Object::Name(ref name) = operations[i] {
            let name_str = String::from_utf8_lossy(name);

            // Check if this is an operator
            // PDF operators: BT, ET, Tf, Td, Tj, TJ, Tm, rg, RG, re, m, l, h, S, f, w, etc.
            // Font names: F1, F1-Bold, etc. (start with F followed by a digit or dash)
            let is_operator = match name_str.as_ref() {
                // Text operators
                "BT" | "ET" | "Tf" | "Td" | "Tj" | "TJ" | "Tm" => true,
                // Color operators
                "rg" | "RG" | "g" | "G" => true,
                // Path construction
                "m" | "l" | "c" | "v" | "y" | "h" | "re" => true,
                // Path painting
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => true,
                // Line width
                "w" => true,
                // Other operators that start with lowercase
                _ if name_str.chars().next().map_or(false, |c| c.is_lowercase()) => true,
                _ => false,
            };

            if is_operator {
                // This is an operator
                let operator = name_str.to_string();
                let mut operands = Vec::new();

                // Collect operands until next operator
                i += 1;
                while i < operations.len() {
                    if let Object::Name(ref next_name) = operations[i] {
                        let next_str = String::from_utf8_lossy(next_name);
                        // Check if this Name is an operator using the same logic
                        let is_next_operator = match next_str.as_ref() {
                            // Text operators
                            "BT" | "ET" | "Tf" | "Td" | "Tj" | "TJ" | "Tm" => true,
                            // Color operators
                            "rg" | "RG" | "g" | "G" => true,
                            // Path construction
                            "m" | "l" | "c" | "v" | "y" | "h" | "re" => true,
                            // Path painting
                            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => true,
                            // Line width
                            "w" => true,
                            // Other operators that start with lowercase
                            _ if next_str.chars().next().map_or(false, |c| c.is_lowercase()) => {
                                true
                            }
                            _ => false,
                        };
                        if is_next_operator {
                            break;
                        }
                    }
                    operands.push(operations[i].clone());
                    i += 1;
                }

                content_ops.push(Operation::new(&operator, operands));
            } else {
                // This Name is an operand, not an operator
                // This shouldn't happen if operations are generated correctly
                trace!(
                    "Warning: Name object '{}' appears without an operator",
                    name_str
                );
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let content = Content {
        operations: content_ops.clone(),
    };

    trace!("Converted {} operations to Content", content_ops.len());
    for op in &content_ops {
        trace!(
            "Operation: {} with operands: {:?}",
            op.operator, op.operands
        );
    }

    // Encode content and add to page
    let content_bytes = content.encode()?;
    doc.add_page_contents(page_id, content_bytes)?;

    Ok(())
}
