//! PDF drawing operations for tables

use crate::PagedTableResult;
use crate::Result;
use crate::TaggedCellHook;
use crate::constants::*;
use crate::drawing_utils::{
    BorderDrawingMode, calculate_cell_width, draw_horizontal_line, draw_rectangle_fill,
    draw_rectangle_stroke, draw_table_borders as draw_borders_util, draw_vertical_line,
    objects_to_operations, set_stroke_style,
};
use crate::layout::TableLayout;
use crate::style::{Alignment, BorderStyle, Color, VerticalAlignment};
use crate::table::{CellImage, Table};
use lopdf::{
    Document, Object, ObjectId, StringFormat,
    content::{Content, Operation},
    dictionary,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, trace};

/// Resource name for the overlay transparency graphics state.
const OVERLAY_GSTATE_NAME: &str = "GSTblOvl";

/// Tracks image XObjects registered in a document for a table draw operation.
pub(crate) struct ImageXObjects {
    /// Maps Arc raw pointer → (resource_name, ObjectId)
    entries: HashMap<*const lopdf::Stream, (String, ObjectId)>,
    counter: usize,
    /// ExtGState ObjectId for overlay transparency (created on demand).
    gstate_id: Option<ObjectId>,
}

impl ImageXObjects {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            counter: 0,
            gstate_id: None,
        }
    }

    /// Register an image in the document if not already registered.
    /// Returns the resource name to reference it with `Do`.
    fn register(&mut self, doc: &mut Document, image: &CellImage) -> String {
        let ptr = Arc::as_ptr(&image.xobject);
        if let Some((name, _)) = self.entries.get(&ptr) {
            return name.clone();
        }
        let name = format!("TblImg{}", self.counter);
        self.counter += 1;
        let obj_id = doc.add_object((*image.xobject).clone());
        self.entries.insert(ptr, (name.clone(), obj_id));
        name
    }

    /// Ensure an ExtGState for 50% opacity overlays exists in the document.
    fn ensure_gstate(&mut self, doc: &mut Document) {
        if self.gstate_id.is_some() {
            return;
        }
        let gs_dict = dictionary! {
            "Type" => "ExtGState",
            "ca" => Object::Real(0.5),
            "CA" => Object::Real(0.5),
        };
        self.gstate_id = Some(doc.add_object(gs_dict));
    }

    /// Register the ExtGState into a page's Resources/ExtGState dictionary.
    fn register_gstate_on_page(&self, doc: &mut Document, page_id: ObjectId) -> Result<()> {
        let gstate_obj_id = match self.gstate_id {
            Some(id) => id,
            None => return Ok(()),
        };

        // Get the Resources dictionary reference from the page
        let resources_ref = if let Ok(Object::Dictionary(page_dict)) = doc.get_object(page_id) {
            match page_dict.get(b"Resources") {
                Ok(Object::Reference(r)) => Some(*r),
                _ => None,
            }
        } else {
            None
        };

        let resources_id = match resources_ref {
            Some(id) => id,
            None => return Ok(()),
        };

        // Get or create ExtGState sub-dictionary on the Resources dictionary
        if let Ok(Object::Dictionary(res_dict)) = doc.get_object_mut(resources_id) {
            match res_dict.get_mut(b"ExtGState") {
                Ok(Object::Dictionary(gs_dict)) => {
                    gs_dict.set(OVERLAY_GSTATE_NAME, gstate_obj_id);
                }
                _ => {
                    let gs_sub = dictionary! {
                        OVERLAY_GSTATE_NAME => gstate_obj_id,
                    };
                    res_dict.set("ExtGState", gs_sub);
                }
            }
        }

        Ok(())
    }

    /// Register all image XObjects (and ExtGState if needed) into a page's Resources.
    pub(crate) fn register_on_page(&self, doc: &mut Document, page_id: ObjectId) -> Result<()> {
        for (_, (name, obj_id)) in &self.entries {
            doc.add_xobject(page_id, name.as_bytes().to_vec(), *obj_id)
                .map_err(|e| {
                    crate::error::TableError::DrawingError(format!(
                        "Failed to register image XObject: {e}"
                    ))
                })?;
        }
        self.register_gstate_on_page(doc, page_id)?;
        Ok(())
    }
}

/// Calculated image render bounds within a cell.
struct ImageRenderBounds {
    img_x: f32,
    img_y: f32,
    render_w: f32,
    render_h: f32,
}

/// Calculate contain-fit image bounds within a cell.
fn calculate_image_render_bounds(
    image: &CellImage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    padding: &crate::style::Padding,
) -> Option<ImageRenderBounds> {
    let available_w = width - padding.left - padding.right;
    let available_h = height - padding.top - padding.bottom;
    if available_w <= 0.0 || available_h <= 0.0 || image.width_px == 0 || image.height_px == 0 {
        return None;
    }

    let aspect = image.aspect_ratio();

    let mut render_h = available_w / aspect;
    if let Some(max_h) = image.max_render_height_pts {
        render_h = render_h.min(max_h);
    }
    render_h = render_h.min(available_h);
    let render_w = render_h * aspect;
    let (render_w, render_h) = if render_w > available_w {
        (available_w, available_w / aspect)
    } else {
        (render_w, render_h)
    };

    let inner_x = x + padding.left;
    let inner_y_bottom = y - height + padding.bottom;
    let img_x = inner_x + (available_w - render_w) / 2.0;
    let img_y = inner_y_bottom + (available_h - render_h) / 2.0;

    Some(ImageRenderBounds {
        img_x,
        img_y,
        render_w,
        render_h,
    })
}

/// Resolve the regular (non-bold) font resource name for overlay text.
fn resolve_overlay_font_resource_name(table: &Table) -> String {
    if let Some(ref name) = table.style.embedded_font_resource_name {
        name.clone()
    } else {
        "F1".to_string()
    }
}

/// Generate PDF objects for drawing an image within a cell,
/// optionally including a text overlay bar.
fn draw_cell_image(
    image: &CellImage,
    resource_name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    padding: &crate::style::Padding,
    table: &Table,
    has_gstate: bool,
) -> Vec<Object> {
    let bounds = match calculate_image_render_bounds(image, x, y, width, height, padding) {
        Some(b) => b,
        None => return Vec::new(),
    };

    let mut objects = vec![
        Object::Name(b"q".to_vec()),
        Object::Name(b"re".to_vec()),
        x.into(),
        (y - height).into(),
        width.into(),
        height.into(),
        Object::Name(b"W".to_vec()),
        Object::Name(b"n".to_vec()),
        Object::Name(b"cm".to_vec()),
        bounds.render_w.into(),
        0.0f32.into(),
        0.0f32.into(),
        bounds.render_h.into(),
        bounds.img_x.into(),
        bounds.img_y.into(),
        Object::Name(b"Do".to_vec()),
        Object::Name(resource_name.as_bytes().to_vec()),
        Object::Name(b"Q".to_vec()),
    ];

    // Draw overlay bar if present
    if let (Some(overlay), true) = (&image.overlay, has_gstate) {
        if !overlay.text.is_empty() {
            objects.extend(draw_image_overlay(overlay, &bounds, table));
        }
    }

    objects
}

/// Render one or more images within a cell, laid out side-by-side with a small gap.
fn draw_cell_images(
    images: &[CellImage],
    registry: &ImageXObjects,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    padding: &crate::style::Padding,
    table: &Table,
) -> Vec<Object> {
    let count = images.len();
    if count == 0 {
        return Vec::new();
    }

    let has_gstate = registry.gstate_id.is_some();

    if count == 1 {
        // Single image — use full cell width (original behaviour)
        let image = &images[0];
        if let Some((name, _)) = registry.entries.get(&Arc::as_ptr(&image.xobject)) {
            return draw_cell_image(image, name, x, y, width, height, padding, table, has_gstate);
        }
        return Vec::new();
    }

    // Multiple images — split available width evenly with a gap between them
    const IMAGE_GAP: f32 = 4.0;
    let available_w = width - padding.left - padding.right;
    let total_gap = IMAGE_GAP * (count as f32 - 1.0);
    let slot_w = (available_w - total_gap) / count as f32;

    let mut objects = Vec::new();
    let mut slot_x = x + padding.left;

    let slot_padding = crate::style::Padding {
        top: padding.top,
        right: 0.0,
        bottom: padding.bottom,
        left: 0.0,
    };

    for image in images {
        if let Some((name, _)) = registry.entries.get(&Arc::as_ptr(&image.xobject)) {
            objects.extend(draw_cell_image(
                image,
                name,
                slot_x,
                y,
                slot_w,
                height,
                &slot_padding,
                table,
                has_gstate,
            ));
        }
        slot_x += slot_w + IMAGE_GAP;
    }

    objects
}

/// Generate PDF objects for a semi-transparent overlay bar with white text
/// at the top of an image.
fn draw_image_overlay(
    overlay: &crate::table::ImageOverlay,
    bounds: &ImageRenderBounds,
    table: &Table,
) -> Vec<Object> {
    let bar_w = bounds.render_w;
    let bar_h = overlay.bar_height.min(bounds.render_h);
    let bar_x = bounds.img_x;
    let bar_y = bounds.img_y + bounds.render_h - bar_h;

    let font_name = resolve_overlay_font_resource_name(table);
    let use_encoded =
        table.style.embedded_font_resource_name.is_some() && table.font_metrics.is_some();

    let text_x = bar_x + overlay.padding;
    let baseline_y = bar_y + bar_h - overlay.font_size - (bar_h - overlay.font_size) / 2.0;

    let mut objects = Vec::new();

    // Semi-transparent black bar
    objects.push(Object::Name(b"q".to_vec()));
    objects.push(Object::Name(b"gs".to_vec()));
    objects.push(Object::Name(OVERLAY_GSTATE_NAME.as_bytes().to_vec()));
    objects.push(Object::Name(b"rg".to_vec()));
    objects.push(0.0f32.into());
    objects.push(0.0f32.into());
    objects.push(0.0f32.into());
    objects.push(Object::Name(b"re".to_vec()));
    objects.push(bar_x.into());
    objects.push(bar_y.into());
    objects.push(bar_w.into());
    objects.push(bar_h.into());
    objects.push(Object::Name(b"f".to_vec()));
    objects.push(Object::Name(b"Q".to_vec()));

    // White text (full opacity, outside the gs scope)
    objects.push(Object::Name(b"BT".to_vec()));
    objects.push(Object::Name(b"rg".to_vec()));
    objects.push(1.0f32.into());
    objects.push(1.0f32.into());
    objects.push(1.0f32.into());
    objects.push(Object::Name(b"Tf".to_vec()));
    objects.push(Object::Name(font_name.as_bytes().to_vec()));
    objects.push(overlay.font_size.into());
    objects.push(Object::Name(b"Td".to_vec()));
    objects.push(text_x.into());
    objects.push(baseline_y.into());

    if use_encoded {
        let encoded = table
            .font_metrics
            .as_ref()
            .unwrap()
            .encode_text(&overlay.text);
        objects.push(Object::Name(b"Tj".to_vec()));
        objects.push(Object::String(encoded, StringFormat::Hexadecimal));
    } else {
        objects.push(Object::Name(b"Tj".to_vec()));
        objects.push(Object::string_literal(overlay.text.clone()));
    }

    objects.push(Object::Name(b"ET".to_vec()));

    objects
}

/// Check whether a table contains any image cells.
pub(crate) fn table_has_images(table: &Table) -> bool {
    table
        .rows
        .iter()
        .any(|row| row.cells.iter().any(|cell| !cell.images.is_empty()))
}

/// Pre-register all unique images from a table into the document.
/// Also creates an ExtGState for overlay transparency when any image has an overlay.
pub(crate) fn register_all_images(doc: &mut Document, table: &Table) -> ImageXObjects {
    let mut registry = ImageXObjects::new();
    let mut has_overlay = false;
    for row in &table.rows {
        for cell in &row.cells {
            for image in &cell.images {
                registry.register(doc, image);
                if image.overlay.is_some() {
                    has_overlay = true;
                }
            }
        }
    }
    if has_overlay {
        registry.ensure_gstate(doc);
    }
    registry
}

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

fn draw_cell_border_overrides(
    cell_style: &crate::style::CellStyle,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Vec<Object> {
    let mut ops = Vec::new();

    // When all four sides share the same style, stroke as a single rectangle so
    // corner joins match the default table grid rendering exactly.
    if let (Some(left), Some(right), Some(top), Some(bottom)) = (
        cell_style.border_left,
        cell_style.border_right,
        cell_style.border_top,
        cell_style.border_bottom,
    ) {
        if left == right && right == top && top == bottom {
            let (style, line_width, color) = top;
            if style != BorderStyle::None {
                ops.extend(set_stroke_style(color, line_width));
                ops.extend(draw_rectangle_stroke(x, y - height, width, height));
            }
            return ops;
        }
    }

    let append_side = |ops: &mut Vec<Object>,
                       border: Option<(BorderStyle, f32, Color)>,
                       side_ops: Vec<Object>| {
        if let Some((style, line_width, color)) = border {
            if style == BorderStyle::None {
                return;
            }
            // Dashed/dotted specific stroking is not yet implemented; treat as visible stroke.
            ops.extend(set_stroke_style(color, line_width));
            ops.extend(side_ops);
        }
    };

    append_side(
        &mut ops,
        cell_style.border_left,
        draw_vertical_line(x, y, y - height),
    );
    append_side(
        &mut ops,
        cell_style.border_right,
        draw_vertical_line(x + width, y, y - height),
    );
    append_side(
        &mut ops,
        cell_style.border_top,
        draw_horizontal_line(x, x + width, y),
    );
    append_side(
        &mut ops,
        cell_style.border_bottom,
        draw_horizontal_line(x, x + width, y - height),
    );

    ops
}

/// Generate PDF operations for drawing a table.
///
/// When `image_registry` is provided, image cells are rendered using the
/// pre-registered XObject resource names. Pass `None` for text-only tables.
pub fn generate_table_operations(
    table: &Table,
    layout: &TableLayout,
    position: (f32, f32),
    mut hook: Option<&mut dyn TaggedCellHook>,
    image_registry: Option<&ImageXObjects>,
) -> Result<Vec<Object>> {
    let mut operations = Vec::new();
    let mut cell_border_overlay_ops = Vec::new();
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
                operations.extend(operations_to_objects(
                    cell_hook.begin_cell(row_idx, col_idx, is_header),
                ));
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

            // Draw cell images if present
            if let Some(registry) = image_registry {
                if !cell.images.is_empty() {
                    let padding = cell
                        .style
                        .as_ref()
                        .and_then(|s| s.padding.as_ref())
                        .unwrap_or(&table.style.padding);
                    operations.extend(draw_cell_images(
                        &cell.images,
                        registry,
                        current_x,
                        current_y,
                        cell_width,
                        row_height,
                        padding,
                        table,
                    ));
                }
            }

            if let Some(cell_hook) = hook.as_deref_mut() {
                operations.extend(operations_to_objects(
                    cell_hook.end_cell(row_idx, col_idx, is_header),
                ));
            }

            // Draw per-cell border overrides after semantic cell content so they remain visual-only.
            if let Some(ref cell_style) = cell.style {
                let cell_border_ops = draw_cell_border_overrides(
                    cell_style, current_x, current_y, cell_width, row_height,
                );
                if artifactize_non_semantic {
                    cell_border_overlay_ops.extend(wrap_objects_as_artifact(cell_border_ops));
                } else {
                    cell_border_overlay_ops.extend(cell_border_ops);
                }
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
    operations.extend(cell_border_overlay_ops);

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
    image_registry: Option<&ImageXObjects>,
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
                    if page_ids.len() == 1 {
                        start_y
                    } else {
                        page_height - top_margin
                    },
                ),
                &mut hook,
                image_registry,
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
            image_registry,
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
    image_registry: Option<&ImageXObjects>,
) -> Result<()> {
    if row_indices.is_empty() {
        return Ok(());
    }

    debug!("Drawing {} rows on page {:?}", row_indices.len(), page_id);

    let mut operations = Vec::new();
    let mut cell_border_overlay_ops = Vec::new();
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
                operations.extend(operations_to_objects(
                    cell_hook.begin_cell(row_idx, col_idx, is_header),
                ));
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

            // Draw cell images if present
            if let Some(registry) = image_registry {
                if !cell.images.is_empty() {
                    let padding = cell
                        .style
                        .as_ref()
                        .and_then(|s| s.padding.as_ref())
                        .unwrap_or(&table.style.padding);
                    operations.extend(draw_cell_images(
                        &cell.images,
                        registry,
                        current_x,
                        current_y,
                        cell_width,
                        row_height,
                        padding,
                        table,
                    ));
                }
            }

            if let Some(cell_hook) = hook.as_deref_mut() {
                operations.extend(operations_to_objects(
                    cell_hook.end_cell(row_idx, col_idx, is_header),
                ));
            }

            // Draw per-cell border overrides after semantic cell content so they remain visual-only.
            if let Some(ref cell_style) = cell.style {
                let cell_border_ops = draw_cell_border_overrides(
                    cell_style, current_x, current_y, cell_width, row_height,
                );
                if artifactize_non_semantic {
                    cell_border_overlay_ops.extend(wrap_objects_as_artifact(cell_border_ops));
                } else {
                    cell_border_overlay_ops.extend(cell_border_ops);
                }
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
    operations.extend(cell_border_overlay_ops);

    // Register image XObjects on this page
    if let Some(registry) = image_registry {
        registry.register_on_page(doc, page_id)?;
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
