# lopdf-table

A composable table drawing library for PDFs built on [lopdf](https://github.com/J-F-Liu/lopdf).

## Features

- **Ergonomic API**: Simple builder pattern for creating tables
- **Automatic Sizing**: Calculates column widths and row heights based on content
- **Flexible Styling**: Customize colors, borders, padding, and alignment
- **Cell Spanning**: Support for colspan and rowspan
- **Trait-based Design**: Extends lopdf's Document type seamlessly

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lopdf-table = "0.1"
```

## Usage

### Basic Table

```rust
use lopdf::Document;
use lopdf_table::{Table, Row, Cell, TableDrawing};

// Create a document
let mut doc = Document::new();

// Create a table
let table = Table::new()
    .add_row(Row::new(vec![
        Cell::new("Name").bold(),
        Cell::new("Age").bold(),
        Cell::new("City").bold(),
    ]))
    .add_row(Row::new(vec![
        Cell::new("Alice"),
        Cell::new("30"),
        Cell::new("New York"),
    ]))
    .add_row(Row::new(vec![
        Cell::new("Bob"),
        Cell::new("25"),
        Cell::new("London"),
    ]))
    .with_border(1.0);

// Draw the table on a page
doc.draw_table(page_id, table, (50.0, 750.0))?;
```

### Styled Table

```rust
use lopdf_table::{Color, CellStyle, RowStyle, Alignment};

// Create header style
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

// Apply styles
let table = Table::new()
    .add_row(
        Row::new(vec![
            Cell::new("Product").with_style(header_cell_style.clone()),
            Cell::new("Price").with_style(header_cell_style.clone()),
        ])
        .with_style(header_style),
    );
```

### Custom Column Widths

```rust
let table = Table::new()
    .add_row(Row::new(vec![...]))
    .with_column_widths(vec![100.0, 200.0, 150.0]);
```

## Examples

See the `examples/` directory for complete working examples:
- `basic_table.rs` - Simple table with headers and data
- `styled_table.rs` - Advanced styling with colors and formatting

Run examples with:
```bash
cargo run --example basic_table
cargo run --example styled_table
```

## License

MIT