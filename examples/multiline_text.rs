//! Example demonstrating multiline text support in table cells

use lopdf::{Document, Object, dictionary};
use lopdf_table::{
    Cell, CellStyle, Color, Row, Table, TableDrawing, VerticalAlignment, style::Padding,
};
use tracing_subscriber;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "debug".into()))
        .init();

    // Create a new PDF document
    let mut doc = Document::with_version("1.5");

    // Create a page
    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => vec![],
        "Count" => 0,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    // Add page to pages
    if let Ok(Object::Dictionary(pages)) = doc.get_object_mut(pages_id) {
        if let Ok(Object::Array(kids)) = pages.get_mut(b"Kids") {
            kids.push(page_id.into());
        }
        pages.set("Count", Object::Integer(1));
    }

    // Add font resources
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });

    let font_bold_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
        "Encoding" => "WinAnsiEncoding",
    });

    // Add resources to page
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
            "F1-Bold" => font_bold_id,
        },
    });

    if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
        page.set("Resources", resources_id);
    }

    // Update catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Create a table demonstrating multiline text
    let table = Table::new()
        .add_row(Row::new(vec![
            Cell::new("Feature").bold(),
            Cell::new("Description").bold(),
            Cell::new("Example").bold(),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Basic\nMultiline"),
            Cell::new("Text with embedded newlines\nis preserved correctly"),
            Cell::new("Line 1\nLine 2\nLine 3"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Multiple\n\nNewlines"),
            Cell::new("Consecutive newlines\n\nCreate blank lines\n\n\nLike this!"),
            Cell::new("Paragraph 1\n\nParagraph 2\n\n\nParagraph 3"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Wrapped\nMultiline").with_wrap(true),
            Cell::new("Long text with newlines that also needs wrapping:\nThis is a very long line that will be wrapped automatically based on the cell width\nShort line\nAnother extremely long line that demonstrates how wrapping works in conjunction with explicit line breaks").with_wrap(true),
            Cell::new("First Section:\nThis text will wrap if it's too long for the cell width\n\nSecond Section:\nMore wrapping demonstration").with_wrap(true),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Formatted\nAddress"),
            Cell::new("John Doe\n123 Main Street\nApartment 4B\nNew York, NY 10001\nUSA"),
            Cell::new("Invoice Address:\n\nAcme Corporation\n456 Business Blvd\nSuite 200\nLos Angeles, CA 90001"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("List\nItems"),
            Cell::new("Shopping List:\n• Apples\n• Bananas\n• Oranges\n• Grapes\n• Strawberries"),
            Cell::new("TODO:\n1. Fix multiline text\n2. Add tests\n3. Create example\n4. Test implementation\n5. Ship it!"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Code\nSnippet"),
            Cell::new("Simple code example:\n\nfn main() {\n    println!(\"Hello, world!\");\n}"),
            Cell::new("Python:\n\ndef greet(name):\n    return f\"Hello, {name}!\"\n\nprint(greet(\"World\"))"),
        ]))
        .with_border(1.0)
        .with_total_width(520.0);

    // Draw the first table
    doc.draw_table(page_id, table, (40.0, 750.0))?;

    // Create a second table with different alignments and styling
    let styled_table = Table::new()
        .add_row(Row::new(vec![
            Cell::new("Alignment Demo").bold(),
            Cell::new("Multiline Content").bold(),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Top\nAligned")
                .with_style(CellStyle {
                    vertical_alignment: VerticalAlignment::Top,
                    padding: Some(Padding::uniform(10.0)),
                    background_color: Some(Color::rgb(0.95, 0.95, 1.0)),
                    ..Default::default()
                }),
            Cell::new("This text is\naligned to the\ntop of the cell\n\nNotice how it\nstays at the top")
                .with_style(CellStyle {
                    vertical_alignment: VerticalAlignment::Top,
                    padding: Some(Padding::uniform(10.0)),
                    ..Default::default()
                }),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Middle\nAligned")
                .with_style(CellStyle {
                    vertical_alignment: VerticalAlignment::Middle,
                    padding: Some(Padding::uniform(10.0)),
                    background_color: Some(Color::rgb(0.95, 1.0, 0.95)),
                    ..Default::default()
                }),
            Cell::new("This text is\ncentered\nvertically\n\nIn the middle\nof the cell")
                .with_style(CellStyle {
                    vertical_alignment: VerticalAlignment::Middle,
                    padding: Some(Padding::uniform(10.0)),
                    ..Default::default()
                }),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Bottom\nAligned")
                .with_style(CellStyle {
                    vertical_alignment: VerticalAlignment::Bottom,
                    padding: Some(Padding::uniform(10.0)),
                    background_color: Some(Color::rgb(1.0, 0.95, 0.95)),
                    ..Default::default()
                }),
            Cell::new("This text is\naligned to the\nbottom of the cell\n\nSee how it sits\nat the bottom")
                .with_style(CellStyle {
                    vertical_alignment: VerticalAlignment::Bottom,
                    padding: Some(Padding::uniform(10.0)),
                    ..Default::default()
                }),
        ]))
        .with_border(1.0)
        .with_total_width(400.0);

    // Draw the second table
    doc.draw_table(page_id, styled_table, (40.0, 280.0))?;

    // Save the PDF
    doc.save("multiline_text.pdf")?;
    println!("PDF saved as 'multiline_text.pdf'");
    println!("This example demonstrates:");
    println!("  - Basic multiline text with \\n characters");
    println!("  - Multiple consecutive newlines for spacing");
    println!("  - Multiline text with word wrapping");
    println!("  - Various vertical alignments with multiline text");
    println!("  - Real-world use cases like addresses and lists");

    Ok(())
}
