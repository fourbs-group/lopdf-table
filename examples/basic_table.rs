//! Basic table example

use lopdf::{Document, Object, dictionary};
use lopdf_table::{Cell, ColumnWidth, Row, Table, TableDrawing};
use tracing_subscriber;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with debug level
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

    // Add font resource
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

    // Create a simple table with mixed column width specifications
    let table = Table::new()
        .add_row(Row::new(vec![
            Cell::new("Product").bold(),
            Cell::new("Quantity").bold(),
            Cell::new("Price").bold(),
            Cell::new("Total").bold(),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Widget A"),
            Cell::new("5"),
            Cell::new("$10.00"),
            Cell::new("$50.00"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Widget B"),
            Cell::new("3"),
            Cell::new("$15.00"),
            Cell::new("$45.00"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Widget C"),
            Cell::new("10"),
            Cell::new("$5.00"),
            Cell::new("$50.00"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Total").bold(),
            Cell::new("").bold(),
            Cell::new("").bold(),
            Cell::new("$145.00").bold(),
        ]))
        .with_column_widths(vec![
            ColumnWidth::Percentage(40.0), // Product takes 40% of table width
            ColumnWidth::Auto,             // Quantity auto-sized based on content
            ColumnWidth::Pixels(80.0),     // Price fixed at 80 pixels
            ColumnWidth::Pixels(80.0),     // Total fixed at 80 pixels
        ])
        .with_total_width(500.0) // Total table width of 500 points
        .with_border(1.0);

    // Draw the table on the page
    doc.draw_table(page_id, table, (50.0, 750.0))?;

    // Save the PDF
    doc.save("basic_table.pdf")?;
    println!("PDF saved as 'basic_table.pdf'");

    Ok(())
}
