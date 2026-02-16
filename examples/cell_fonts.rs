/// Example demonstrating cell-level font customization
use lopdf::{Document, Object, dictionary};
use lopdf_table::{Cell, CellStyle, Row, Table, TableDrawing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new document
    let mut doc = Document::new();

    // Create a new page
    let page_id = doc.new_object_id();
    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => Vec::<Object>::new(),
        "Resources" => dictionary! {
            "ProcSet" => vec!["PDF".into(), "Text".into()],
        },
    };
    doc.objects.insert(page_id, Object::Dictionary(page_dict));

    // Add pages catalog
    let pages_id = doc.new_object_id();
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Update page parent
    if let Some(Object::Dictionary(page)) = doc.objects.get_mut(&page_id) {
        page.set("Parent", pages_id);
    }

    // Set catalog
    let catalog_id = doc.new_object_id();
    let catalog_dict = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    doc.objects
        .insert(catalog_id, Object::Dictionary(catalog_dict));
    doc.trailer.set("Root", catalog_id);

    // Add font resources
    // Helvetica fonts (default)
    let font_helvetica = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let font_helvetica_bold = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
        "Encoding" => "WinAnsiEncoding",
    });

    // Courier fonts
    let font_courier = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
        "Encoding" => "WinAnsiEncoding",
    });
    let font_courier_bold = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier-Bold",
        "Encoding" => "WinAnsiEncoding",
    });

    // Times-Roman fonts
    let font_times = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Times-Roman",
        "Encoding" => "WinAnsiEncoding",
    });
    let font_times_bold = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Times-Bold",
        "Encoding" => "WinAnsiEncoding",
    });

    // Add resources to page
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_helvetica,
            "F1-Bold" => font_helvetica_bold,
            "F2" => font_courier,
            "F2-Bold" => font_courier_bold,
            "F3" => font_times,
            "F3-Bold" => font_times_bold,
        },
    });

    if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
        page.set("Resources", resources_id);
    }

    // Create cell styles
    let serial_style = CellStyle {
        font_name: Some("Courier".to_string()),
        ..Default::default()
    };

    let times_style = CellStyle {
        font_name: Some("Times-Roman".to_string()),
        bold: true,
        ..Default::default()
    };

    let courier_bold_style = CellStyle {
        font_name: Some("Courier".to_string()),
        bold: true,
        ..Default::default()
    };

    // Create table demonstrating font hierarchy
    let table = Table::new()
        // Header row - using default table font (Helvetica)
        .add_row(Row::new(vec![
            Cell::new("Product").bold(),
            Cell::new("Serial Number").bold(),
            Cell::new("Description").bold(),
            Cell::new("Price").bold(),
        ]))
        // Row 1: Serial number in Courier font
        .add_row(Row::new(vec![
            Cell::new("Laptop"),
            Cell::new("LAP-2024-001-XYZ").with_style(serial_style.clone()),
            Cell::new("High-performance laptop with 16GB RAM"),
            Cell::new("$1,299.00"),
        ]))
        // Row 2: Product name in Times-Roman, Serial in Courier
        .add_row(Row::new(vec![
            Cell::new("Desktop PC").with_style(times_style),
            Cell::new("DSK-2024-002-ABC").with_style(serial_style.clone()),
            Cell::new("Gaming desktop with RTX 4090"),
            Cell::new("$2,499.00"),
        ]))
        // Row 3: Mixed fonts in different cells
        .add_row(Row::new(vec![
            Cell::new("Monitor"),
            Cell::new("MON-2024-003-QWE").with_style(serial_style.clone()),
            Cell::new("4K UHD Display").with_style(courier_bold_style),
            Cell::new("$599.00"),
        ]));

    // Draw the table
    let position = (50.0, 750.0);
    doc.draw_table(page_id, table, position)?;

    // Save the document
    doc.save("cell_fonts_example.pdf")?;
    println!("PDF created successfully: cell_fonts_example.pdf");

    Ok(())
}
