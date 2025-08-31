//! Multi-page table example demonstrating automatic page wrapping and header repetition

use lopdf::{Document, Object, dictionary};
use lopdf_table::{
    Alignment, Cell, CellStyle, Color, ColumnWidth, PagedTableResult, Row, RowStyle, Table,
    TableDrawing,
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

    // Create the Pages object
    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => vec![],
        "Count" => 0,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    // Create the first page
    let first_page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });

    // Add first page to pages
    if let Ok(Object::Dictionary(pages)) = doc.get_object_mut(pages_id) {
        if let Ok(Object::Array(kids)) = pages.get_mut(b"Kids") {
            kids.push(first_page_id.into());
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

    if let Ok(Object::Dictionary(page)) = doc.get_object_mut(first_page_id) {
        page.set("Resources", resources_id);
    }

    // Update catalog
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Create a large table that will span multiple pages
    let mut table = Table::new()
        .with_border(1.0)
        .with_header_rows(2) // First 2 rows are headers
        .with_column_widths(vec![
            ColumnWidth::Pixels(60.0),  // ID column
            ColumnWidth::Pixels(150.0), // Name column
            ColumnWidth::Auto,          // Department
            ColumnWidth::Pixels(100.0), // Position
            ColumnWidth::Pixels(80.0),  // Salary
        ])
        .with_total_width(500.0);

    // Header row style
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

    // Add main header row (create 5 cells for proper validation, even though we'll use colspan later)
    table = table.add_row(
        Row::new(vec![
            Cell::new("Employee Data Report").with_style(CellStyle {
                text_color: Color::white(),
                bold: true,
                alignment: Alignment::Center,
                ..Default::default()
            }),
            Cell::empty(),
            Cell::empty(),
            Cell::empty(),
            Cell::empty(),
        ])
        .with_style(RowStyle {
            background_color: Some(Color::rgb(0.15, 0.25, 0.45)),
            ..Default::default()
        }),
    );

    // Add column headers
    table = table.add_row(
        Row::new(vec![
            Cell::new("ID").with_style(header_cell_style.clone()),
            Cell::new("Employee Name").with_style(header_cell_style.clone()),
            Cell::new("Department").with_style(header_cell_style.clone()),
            Cell::new("Position").with_style(header_cell_style.clone()),
            Cell::new("Salary").with_style(header_cell_style.clone()),
        ])
        .with_style(header_style),
    );

    // Generate many data rows to span multiple pages
    let departments = vec![
        "Engineering",
        "Marketing",
        "Sales",
        "HR",
        "Finance",
        "Operations",
    ];
    let positions = vec![
        "Manager",
        "Senior Developer",
        "Developer",
        "Analyst",
        "Specialist",
        "Coordinator",
    ];
    let first_names = vec![
        "John", "Jane", "Bob", "Alice", "Charlie", "Diana", "Eve", "Frank",
    ];
    let last_names = vec![
        "Smith", "Johnson", "Williams", "Brown", "Davis", "Miller", "Wilson", "Moore",
    ];

    for i in 1..=50 {
        let first_name = first_names[(i - 1) % first_names.len()];
        let last_name = last_names[(i * 3) % last_names.len()];
        let department = departments[(i * 2) % departments.len()];
        let position = positions[(i * 5) % positions.len()];
        let salary = 50000 + (i * 2500) % 50000;

        // Alternate row colors for better readability
        let row_style = if i % 2 == 0 {
            Some(RowStyle {
                background_color: Some(Color::rgb(0.95, 0.95, 0.95)),
                ..Default::default()
            })
        } else {
            None
        };

        let mut row = Row::new(vec![
            Cell::new(format!("E{:03}", i)),
            Cell::new(format!("{} {}", first_name, last_name)),
            Cell::new(department),
            Cell::new(position),
            Cell::new(format!("${}", salary)).with_style(CellStyle {
                alignment: Alignment::Right,
                ..Default::default()
            }),
        ]);

        if let Some(style) = row_style {
            row = row.with_style(style);
        }

        table = table.add_row(row);
    }

    // Draw the table with pagination
    println!("Drawing multi-page table...");
    let result: PagedTableResult =
        doc.draw_table_with_pagination(first_page_id, table, (50.0, 750.0))?;

    println!("Table drawn across {} pages", result.total_pages);
    println!("Page IDs used: {:?}", result.page_ids);
    println!("Final position: {:?}", result.final_position);

    // Save the PDF
    doc.save("multi_page_table.pdf")?;
    println!("PDF saved as 'multi_page_table.pdf'");

    Ok(())
}
