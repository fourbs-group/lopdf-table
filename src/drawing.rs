//! PDF drawing operations for tables

use crate::PagedTableResult;
use crate::Result;
use crate::TaggedCellHook;
use crate::constants::*;
use crate::drawing_utils::{
    BorderDrawingMode, calculate_cell_width, draw_rectangle_fill,
    draw_table_borders as draw_borders_util, objects_to_operations,
};
use crate::layout::TableLayout;
use crate::style::{Alignment, Color, VerticalAlignment};
use crate::table::Table;
use lopdf::{
    Document, Object, ObjectId, StringFormat,
    content::{Content, Operation},
    dictionary,
};
use tracing::{debug, trace};

fn wrap_objects_as_artifact(mut objects: Vec<Object>) -> Vec<Object> {
    if objects.is_empty() {
        return objects;
    }

    let mut wrapped = Vec::with_capacity(objects.len() + 4);
    wrapped.push(Object::Name(b"BDC".to_vec()));
    wrapped.push(Object::Name(b"Artifact".to_vec()));
    wrapped.push(Object::Dictionary(dictionary! {
        "Type" => Object::Name(b"Layout".to_vec()),
    }));
    wrapped.append(&mut objects);
    wrapped.push(Object::Name(b"EMC".to_vec()));
    wrapped
}

/// Generate PDF operations for drawing a table
pub fn generate_table_operations(
    table: &Table,
    layout: &TableLayout,
    position: (f32, f32),
    mut hook: Option<&mut dyn TaggedCellHook>,
) -> Result<Vec<Object>> {
    let mut operations = Vec::new();
    let (start_x, start_y) = position;
    let artifactize_non_semantic = hook.is_some();

    debug!(
        "Generating operations for table at ({}, {})",
        start_x, start_y
    );

    // Draw table background if specified
    if let Some(bg_color) = &table.style.background_color {
        let bg_ops = draw_rectangle_fill(
            start_x,
            start_y - layout.total_height,
            layout.total_width,
            layout.total_height,
            *bg_color,
        );
        if artifactize_non_semantic {
            operations.extend(wrap_objects_as_artifact(bg_ops));
        } else {
            operations.extend(bg_ops);
        }
    }

    // Draw cells and content
    let mut current_y = start_y;

    for (row_idx, row) in table.rows.iter().enumerate() {
        let row_height = layout.row_heights[row_idx];
        let mut current_x = start_x;

        // Draw row background if specified
        if let Some(ref row_style) = row.style {
            if let Some(bg_color) = row_style.background_color {
                let row_bg_ops = draw_rectangle_fill(
                    start_x,
                    current_y - row_height,
                    layout.total_width,
                    row_height,
                    bg_color,
                );
                if artifactize_non_semantic {
                    operations.extend(wrap_objects_as_artifact(row_bg_ops));
                } else {
                    operations.extend(row_bg_ops);
                }
            }
        }

        let mut col_idx = 0;
        for cell in row.cells.iter() {
            if col_idx >= layout.column_widths.len() {
                break;
            }
            let is_header = row_idx < table.header_rows;

            if let Some(cell_hook) = hook.as_deref_mut() {
                operations.extend(operations_to_objects(cell_hook.begin_cell(
                    row_idx, col_idx, is_header,
                )));
            }

            // Calculate the total width for cells with colspan
            let cell_width = calculate_cell_width(col_idx, cell.colspan, &layout.column_widths);

            // Draw cell background if specified
            if let Some(ref cell_style) = cell.style {
                if let Some(bg_color) = cell_style.background_color {
                    operations.extend(draw_rectangle_fill(
                        current_x,
                        current_y - row_height,
                        cell_width,
                        row_height,
                        bg_color,
                    ));
                }
            }

            // Draw cell content (text)
            operations.extend(draw_cell_text(
                cell, table, current_x, current_y, cell_width, row_height,
            )?);

            if let Some(cell_hook) = hook.as_deref_mut() {
                operations.extend(operations_to_objects(cell_hook.end_cell(
                    row_idx, col_idx, is_header,
                )));
            }

            current_x += cell_width;
            col_idx += cell.colspan.max(1);
        }

        current_y -= row_height;
    }

    // Draw table borders
    let border_ops = draw_table_borders(table, layout, position);
    if artifactize_non_semantic {
        operations.extend(wrap_objects_as_artifact(border_ops));
    } else {
        operations.extend(border_ops);
    }

    trace!("Generated {} operations", operations.len());
    Ok(operations)
}

/// Draw table borders (wrapper for the shared utility)
fn draw_table_borders(table: &Table, layout: &TableLayout, position: (f32, f32)) -> Vec<Object> {
    draw_borders_util(table, layout, position, BorderDrawingMode::Full, None)
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

/// Estimate text width, using selected font metrics when available
fn measure_text_width(
    text: &str,
    font_size: f32,
    metrics: Option<&dyn crate::font::FontMetrics>,
) -> f32 {
    if let Some(metrics) = metrics {
        metrics.text_width(text, font_size)
    } else {
        text.chars().count() as f32 * font_size * DEFAULT_CHAR_WIDTH_RATIO
    }
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

    // Calculate available width for text
    let available_width = width - padding.left - padding.right;
    let metrics = metrics_for_cell(table, cell);
    let is_bold = cell_is_bold(cell);

    // Wrap text if enabled
    let lines = if cell.text_wrap {
        if let Some(metrics) = metrics {
            crate::text::wrap_text_with_metrics(&cell.content, available_width, font_size, metrics)
        } else {
            crate::text::wrap_text(&cell.content, available_width, font_size)
        }
    } else {
        // Split by newlines even when wrapping is off, to handle embedded newlines
        cell.content.split('\n').map(|s| s.to_string()).collect()
    };

    // Calculate line height
    let line_height = font_size * DEFAULT_LINE_HEIGHT_MULTIPLIER;
    let total_text_height = lines.len() as f32 * line_height;

    // Calculate starting Y position based on vertical alignment
    let start_y = match v_alignment {
        VerticalAlignment::Top => y - padding.top - font_size,
        VerticalAlignment::Middle => y - height / 2.0 + total_text_height / 2.0 - font_size,
        VerticalAlignment::Bottom => y - height + padding.bottom + total_text_height - font_size,
    };

    // Begin text object
    operations.push(Operation::new("BT", vec![]));

    // Determine which font resource name to use:
    // 1. Cell's embedded_font_resource_name (if set)
    // 2. Table style's embedded_font_resource_name_bold when bold (if set)
    // 3. Table style's embedded_font_resource_name (if set)
    // 4. Type1 font mapping (backward compatible)
    let embedded_font_name = cell
        .style
        .as_ref()
        .and_then(|s| s.embedded_font_resource_name.as_deref())
        .or_else(|| {
            if is_bold {
                table
                    .style
                    .embedded_font_resource_name_bold
                    .as_deref()
                    .or(table.style.embedded_font_resource_name.as_deref())
            } else {
                table.style.embedded_font_resource_name.as_deref()
            }
        });

    let use_encoded_text = embedded_font_name.is_some() && metrics.is_some();

    let font_resource_name: String = if let Some(efn) = embedded_font_name {
        efn.to_string()
    } else {
        // Type1 font mapping
        let base_font_name = cell
            .style
            .as_ref()
            .and_then(|s| s.font_name.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(&table.style.font_name);

        if is_bold {
            match base_font_name {
                "Helvetica" => "F1-Bold",
                "Courier" => "F2-Bold",
                "Times-Roman" => "F3-Bold",
                _ => "F1-Bold",
            }
        } else {
            match base_font_name {
                "Helvetica" => "F1",
                "Courier" => "F2",
                "Times-Roman" => "F3",
                _ => "F1",
            }
        }
        .to_string()
    };

    operations.push(Operation::new(
        "Tf",
        vec![
            Object::Name(font_resource_name.as_bytes().to_vec()),
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

    // Position to the first line
    let first_line_y = start_y;

    // Draw each line of text
    for (line_idx, line) in lines.iter().enumerate() {
        let estimated_text_width = measure_text_width(line, font_size, metrics);

        let text_x = match alignment {
            Alignment::Left => x + padding.left,
            Alignment::Center => x + width / 2.0 - estimated_text_width / 2.0,
            Alignment::Right => x + width - padding.right - estimated_text_width,
        };

        let text_y = first_line_y - (line_idx as f32 * line_height);

        if line_idx == 0 {
            operations.push(Operation::new("Td", vec![text_x.into(), text_y.into()]));
        } else {
            let prev_line = &lines[line_idx - 1];
            let prev_width = measure_text_width(prev_line, font_size, metrics);
            let prev_x = match alignment {
                Alignment::Left => x + padding.left,
                Alignment::Center => x + width / 2.0 - prev_width / 2.0,
                Alignment::Right => x + width - padding.right - prev_width,
            };

            let dx = text_x - prev_x;
            let dy = -line_height;
            operations.push(Operation::new("Td", vec![dx.into(), dy.into()]));
        }

        // Show text: use glyph ID encoding for embedded fonts, string literal for Type1
        if use_encoded_text {
            let encoded_bytes = metrics.unwrap().encode_text(line);
            operations.push(Operation::new(
                "Tj",
                vec![Object::String(encoded_bytes, StringFormat::Hexadecimal)],
            ));
        } else {
            operations.push(Operation::new(
                "Tj",
                vec![Object::string_literal(line.clone())],
            ));
        }
    }

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
    // Convert text drawing operations to the flat Object list and
    // wrap them with a clipping path equal to the cell bounds so that
    // text never renders outside the cell.
    let ops = draw_cell_text_operations(cell, table, x, y, width, height);
    let mut objects = Vec::new();

    // Save graphics state
    objects.push(Object::Name("q".as_bytes().to_vec()));
    // Define clipping rectangle (PDF uses lower-left origin)
    objects.push(Object::Name("re".as_bytes().to_vec()));
    objects.push(x.into());
    objects.push((y - height).into());
    objects.push(width.into());
    objects.push(height.into());
    // Set clip and end path
    objects.push(Object::Name("W".as_bytes().to_vec()));
    objects.push(Object::Name("n".as_bytes().to_vec()));

    // Emit text operations
    for op in ops {
        objects.push(Object::Name(op.operator.as_bytes().to_vec()));
        objects.extend(op.operands);
    }

    // Restore graphics state
    objects.push(Object::Name("Q".as_bytes().to_vec()));

    Ok(objects)
}

fn operations_to_objects(ops: Vec<Operation>) -> Vec<Object> {
    let mut out = Vec::new();
    for op in ops {
        out.push(Object::Name(op.operator.as_bytes().to_vec()));
        out.extend(op.operands);
    }
    out
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

    // Convert operations to Content using the shared utility
    let content_ops = objects_to_operations(&operations);
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

/// Draw a table with pagination support
pub fn draw_table_paginated(
    doc: &mut Document,
    start_page_id: ObjectId,
    table: &Table,
    layout: &TableLayout,
    position: (f32, f32),
    mut hook: Option<&mut dyn TaggedCellHook>,
) -> Result<PagedTableResult> {
    debug!(
        "Drawing paginated table with {} rows, {} header rows",
        table.rows.len(),
        table.header_rows
    );

    // Get page dimensions
    let page_height = table.style.page_height.unwrap_or(A4_HEIGHT); // A4 default
    let top_margin = table.style.top_margin;
    let bottom_margin = table.style.bottom_margin;

    let (start_x, start_y) = position;
    let _available_height = start_y - bottom_margin;

    // Track pages used
    let mut page_ids = vec![start_page_id];
    let mut current_page_id = start_page_id;
    let mut current_y = start_y;
    let mut rows_on_current_page = Vec::new();

    // Process all rows
    let mut row_idx = 0;
    while row_idx < table.rows.len() {
        let row_height = layout.row_heights[row_idx];

        // Check if this row fits on the current page
        if current_y - row_height < bottom_margin && !rows_on_current_page.is_empty() {
            // Draw rows accumulated for current page
            draw_rows_subset(
                doc,
                current_page_id,
                table,
                layout,
                &rows_on_current_page,
                (
                    start_x,
                    if rows_on_current_page[0] < table.header_rows {
                        start_y
                    } else {
                        page_height - top_margin
                    },
                ),
                &mut hook,
            )?;

            // Create new page
            current_page_id = create_new_page(doc, current_page_id)?;
            page_ids.push(current_page_id);

            // Reset position for new page
            current_y = page_height - top_margin;
            rows_on_current_page.clear();

            // Add header rows to new page if configured
            if table.style.repeat_headers && table.header_rows > 0 && row_idx >= table.header_rows {
                for header_idx in 0..table.header_rows {
                    rows_on_current_page.push(header_idx);
                    current_y -= layout.row_heights[header_idx];
                }
            }
        }

        // Add current row to page
        rows_on_current_page.push(row_idx);
        current_y -= row_height;
        row_idx += 1;
    }

    // Draw remaining rows on last page
    if !rows_on_current_page.is_empty() {
        let page_y = if page_ids.len() == 1 {
            start_y
        } else {
            page_height - top_margin
        };

        draw_rows_subset(
            doc,
            current_page_id,
            table,
            layout,
            &rows_on_current_page,
            (start_x, page_y),
            &mut hook,
        )?;
    }

    Ok(PagedTableResult {
        total_pages: page_ids.len(),
        page_ids,
        final_position: (start_x, current_y),
    })
}

/// Create a new page with the same configuration as the source page
fn create_new_page(doc: &mut Document, source_page_id: ObjectId) -> Result<ObjectId> {
    debug!("Creating new page for table continuation");

    // Get the parent Pages object from the source page
    let pages_id = if let Ok(Object::Dictionary(page_dict)) = doc.get_object(source_page_id) {
        if let Ok(Object::Reference(pages_ref)) = page_dict.get(b"Parent") {
            *pages_ref
        } else {
            return Err(crate::error::TableError::DrawingError(
                "Could not find parent Pages object".to_string(),
            ));
        }
    } else {
        return Err(crate::error::TableError::DrawingError(
            "Invalid page object".to_string(),
        ));
    };

    // Get MediaBox and Resources from source page
    let (media_box, resources_id) =
        if let Ok(Object::Dictionary(page_dict)) = doc.get_object(source_page_id) {
            let media_box = page_dict.get(b"MediaBox").ok().cloned();
            let resources = page_dict.get(b"Resources").ok().cloned();
            (media_box, resources)
        } else {
            (None, None)
        };

    // Create new page dictionary
    let mut new_page_dict = dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
    };

    if let Some(media_box) = media_box {
        new_page_dict.set("MediaBox", media_box);
    } else {
        // Default to A4
        new_page_dict.set(
            "MediaBox",
            vec![0.into(), 0.into(), A4_WIDTH.into(), A4_HEIGHT.into()],
        );
    }

    if let Some(resources) = resources_id {
        new_page_dict.set("Resources", resources);
    }

    let new_page_id = doc.add_object(new_page_dict);

    // Add page to Pages kids array
    if let Ok(Object::Dictionary(pages_dict)) = doc.get_object_mut(pages_id) {
        if let Ok(Object::Array(kids)) = pages_dict.get_mut(b"Kids") {
            kids.push(new_page_id.into());
        }

        // Update page count
        if let Ok(Object::Integer(count)) = pages_dict.get(b"Count") {
            pages_dict.set("Count", Object::Integer(count + 1));
        }
    }

    trace!("Created new page {:?}", new_page_id);
    Ok(new_page_id)
}

/// Draw a subset of rows on a specific page
fn draw_rows_subset(
    doc: &mut Document,
    page_id: ObjectId,
    table: &Table,
    layout: &TableLayout,
    row_indices: &[usize],
    position: (f32, f32),
    hook: &mut Option<&mut dyn TaggedCellHook>,
) -> Result<()> {
    if row_indices.is_empty() {
        return Ok(());
    }

    debug!("Drawing {} rows on page {:?}", row_indices.len(), page_id);

    let mut operations = Vec::new();
    let (start_x, start_y) = position;
    let mut current_y = start_y;
    let artifactize_non_semantic = hook.is_some();

    // Calculate which columns to draw (all columns for now)
    let column_count = table.column_count();

    // Draw table background if this is the first page
    if row_indices.contains(&0) {
        if let Some(bg_color) = &table.style.background_color {
            let subset_height: f32 = row_indices.iter().map(|&i| layout.row_heights[i]).sum();
            let bg_ops = draw_rectangle_fill(
                start_x,
                start_y - subset_height,
                layout.total_width,
                subset_height,
                *bg_color,
            );
            if artifactize_non_semantic {
                operations.extend(wrap_objects_as_artifact(bg_ops));
            } else {
                operations.extend(bg_ops);
            }
        }
    }

    // Draw rows
    for &row_idx in row_indices {
        let row = &table.rows[row_idx];
        let row_height = layout.row_heights[row_idx];
        let mut current_x = start_x;

        // Draw row background if specified
        if let Some(ref row_style) = row.style {
            if let Some(bg_color) = row_style.background_color {
                let row_bg_ops = draw_rectangle_fill(
                    start_x,
                    current_y - row_height,
                    layout.total_width,
                    row_height,
                    bg_color,
                );
                if artifactize_non_semantic {
                    operations.extend(wrap_objects_as_artifact(row_bg_ops));
                } else {
                    operations.extend(row_bg_ops);
                }
            }
        }

        // Draw cells
        let mut col_idx = 0;
        for cell in row.cells.iter() {
            if col_idx >= column_count {
                break;
            }
            let is_header = row_idx < table.header_rows;

            if let Some(cell_hook) = hook.as_deref_mut() {
                operations.extend(operations_to_objects(cell_hook.begin_cell(
                    row_idx, col_idx, is_header,
                )));
            }

            // Calculate the total width for cells with colspan
            let cell_width = calculate_cell_width(col_idx, cell.colspan, &layout.column_widths);

            // Draw cell background if specified
            if let Some(ref cell_style) = cell.style {
                if let Some(bg_color) = cell_style.background_color {
                    operations.extend(draw_rectangle_fill(
                        current_x,
                        current_y - row_height,
                        cell_width,
                        row_height,
                        bg_color,
                    ));
                }
            }

            // Draw cell content
            operations.extend(draw_cell_text(
                cell, table, current_x, current_y, cell_width, row_height,
            )?);

            if let Some(cell_hook) = hook.as_deref_mut() {
                operations.extend(operations_to_objects(cell_hook.end_cell(
                    row_idx, col_idx, is_header,
                )));
            }

            current_x += cell_width;
            col_idx += cell.colspan.max(1);
        }

        current_y -= row_height;
    }

    // Draw borders for this subset
    let border_ops = draw_subset_borders(table, layout, row_indices, position);
    if artifactize_non_semantic {
        operations.extend(wrap_objects_as_artifact(border_ops));
    } else {
        operations.extend(border_ops);
    }

    // Add operations to page
    add_operations_to_page(doc, page_id, operations)?;

    Ok(())
}

/// Draw borders for a subset of rows (wrapper for the shared utility)
fn draw_subset_borders(
    table: &Table,
    layout: &TableLayout,
    row_indices: &[usize],
    position: (f32, f32),
) -> Vec<Object> {
    if row_indices.is_empty() {
        return Vec::new();
    }
    let subset_height: f32 = row_indices.iter().map(|&i| layout.row_heights[i]).sum();
    draw_borders_util(
        table,
        layout,
        position,
        BorderDrawingMode::Subset(subset_height),
        Some(row_indices),
    )
}
