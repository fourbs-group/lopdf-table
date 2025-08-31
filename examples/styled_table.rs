//! Styled table example with colors and custom formatting

use lopdf::{Document, Object, dictionary};
use lopdf_table::{
    Alignment, Cell, CellStyle, Color, ColumnWidth, Row, RowStyle, Table, TableDrawing, TableStyle,
};
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

    // Add font resources
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let font_bold_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
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

    // Create a styled table with custom colors
    let mut table_style = TableStyle::default();
    table_style.border_width = 2.0;
    table_style.border_color = Color::rgb(0.2, 0.3, 0.5);
    table_style.default_font_size = 11.0;

    // Create header row with custom style
    let header_style = RowStyle {
        background_color: Some(Color::rgb(0.2, 0.3, 0.5)),
        ..Default::default()
    };

    let header_cell_style = CellStyle {
        text_color: Color::white(),
        bold: true,
        alignment: Alignment::Center,
        ..Default::default()
    };

    let table = Table::new()
        .with_style(table_style)
        .add_row(
            Row::new(vec![
                Cell::new("Employee").with_style(header_cell_style.clone()),
                Cell::new("Department").with_style(header_cell_style.clone()),
                Cell::new("Position").with_style(header_cell_style.clone()),
                Cell::new("Salary").with_style(header_cell_style.clone()),
            ])
            .with_style(header_style),
        )
        .add_row(Row::new(vec![
            Cell::new("John Doe"),
            Cell::new("Engineering"),
            Cell::new("Senior Developer"),
            Cell::new("$95,000").with_style(CellStyle {
                alignment: Alignment::Right,
                ..Default::default()
            }),
        ]))
        .add_row(
            Row::new(vec![
                Cell::new("Jane Smith"),
                Cell::new("Marketing"),
                Cell::new("Marketing Manager"),
                Cell::new("$85,000").with_style(CellStyle {
                    alignment: Alignment::Right,
                    ..Default::default()
                }),
            ])
            .with_style(RowStyle {
                background_color: Some(Color::rgb(0.95, 0.95, 0.95)),
                ..Default::default()
            }),
        )
        .add_row(Row::new(vec![
            Cell::new("Bob Johnson"),
            Cell::new("Sales"),
            Cell::new("Sales Representative"),
            Cell::new("$65,000").with_style(CellStyle {
                alignment: Alignment::Right,
                ..Default::default()
            }),
        ]))
        .add_row(
            Row::new(vec![
                Cell::new("Alice Brown"),
                Cell::new("Engineering"),
                Cell::new("Junior Developer"),
                Cell::new("$70,000").with_style(CellStyle {
                    alignment: Alignment::Right,
                    ..Default::default()
                }),
            ])
            .with_style(RowStyle {
                background_color: Some(Color::rgb(0.95, 0.95, 0.95)),
                ..Default::default()
            }),
        )
        .add_row(Row::new(vec![
            Cell::new("Charlie Wilson"),
            Cell::new("HR"),
            Cell::new("HR Specialist"),
            Cell::new("$60,000").with_style(CellStyle {
                alignment: Alignment::Right,
                ..Default::default()
            }),
        ]))
        .with_pixel_widths(vec![120.0, 100.0, 120.0, 80.0]);

    // Draw the table on the page
    doc.draw_table(page_id, table, (50.0, 750.0))?;

    // Create a second table demonstrating text wrapping
    let summary_table = Table::new()
        .add_row(Row::new(vec![
            Cell::new("Department").bold(),
            Cell::new("Description").bold(),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Engineering"),
            Cell::new("The Engineering department focuses on product development, software architecture, and maintaining our technical infrastructure. This team works on both frontend and backend systems.").with_wrap(true),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Marketing"),
            Cell::new("Marketing drives brand awareness and customer acquisition through various channels including digital marketing, content creation, and strategic partnerships.").with_wrap(true),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Sales"),
            Cell::new("Sales team manages client relationships and drives revenue growth through direct sales and account management.").with_wrap(true),
        ]))
        .add_row(Row::new(vec![
            Cell::new("HR"),
            Cell::new("Human Resources handles recruitment, employee relations, benefits administration, and organizational development.").with_wrap(true),
        ]))
        .with_border(1.0)
        .with_column_widths(vec![
            ColumnWidth::Pixels(100.0),       // Department column fixed width
            ColumnWidth::Percentage(60.0),    // Description takes 60% of remaining width
        ])
        .with_total_width(400.0);

    // Draw the second table below the first
    doc.draw_table(page_id, summary_table, (50.0, 450.0))?;

    // Save the PDF
    doc.save("styled_table.pdf")?;
    println!("PDF saved as 'styled_table.pdf'");

    Ok(())
}
