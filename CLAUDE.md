# lopdf-table: Composable Table Drawing Library for PDF

## Project Description
A composable utility library built on top of lopdf that provides ergonomic, fully-featured table drawing capabilities for PDF generation. The library implements a custom trait on lopdf's Document type for seamless integration, supporting automatic column/row sizing, custom styling for rows/columns/cells, and comprehensive border/background customization.

## Implementation Plan

### Architecture Design

**Logging Strategy**
- Use `tracing` library for structured logging
- Log key operations at debug/trace level (table creation, layout calculations)
- Log errors and warnings for invalid inputs or PDF generation issues
- Avoid excessive logging in hot paths (cell iteration, coordinate calculations)

### Architecture Design

**Core Trait Extension**
- Create a `TableDrawing` trait that extends `lopdf::Document`
- Key methods: `draw_table()`, `add_table_to_page()`, `create_table_content()`

**Table Structure Components**
- `Table` struct: Container for rows, columns, and global styling
- `Row` struct: Contains cells and row-specific styling
- `Cell` struct: Content container with colspan, rowspan, and cell styling
- `TableStyle` struct: Manages borders, colors, padding, alignment

**Key Features**
1. **Automatic Sizing**: Calculate column widths and row heights based on content
2. **Flexible Styling**: Cell/row/column background colors, border styles, text colors
3. **Text Handling**: Automatic text wrapping and alignment within cells
4. **Composable API**: Builder pattern for intuitive table construction

### File Structure
```
src/
├── lib.rs           # Main module with trait definition
├── table.rs         # Table, Row, Cell structures
├── style.rs         # Styling structures and enums
├── drawing.rs       # PDF operator generation
├── layout.rs        # Auto-sizing and layout calculations
└── text.rs          # Text measurement and wrapping

examples/
├── basic_table.rs   # Simple table example
└── styled_table.rs  # Complex styling example
```

### Example API Usage
```rust
use lopdf::Document;
use lopdf_table::{TableDrawing, Table, Cell};

let mut doc = Document::new();
let table = Table::new()
    .add_row(vec![
        Cell::new("Header 1").bold(),
        Cell::new("Header 2").bold()
    ])
    .add_row(vec![
        Cell::new("Data 1"),
        Cell::new("Data 2")
    ])
    .with_border(1.0);

doc.draw_table(page_id, table, (x, y));
```

## Key Research Findings

### lopdf Core Components

**Document Structure**
- `Document` struct contains: version, trailer, reference_table, objects, max_id
- Documents are collections of PDF objects stored in a BTreeMap
- Pages are added as dictionary objects with specific Type and MediaBox

**Content Operations**
- `Content` struct holds a vector of `Operation` objects
- `Operation` has two fields: `operator` (String) and `operands` (Vec<Object>)
- Content streams are encoded to bytes and added to page objects

**Object Types**
- `Object` enum includes: Null, Boolean, Integer, Real, Name, String, Array, Dictionary, Stream, Reference
- Objects are the fundamental building blocks of PDF structure

### PDF Drawing Operators

**Path Construction**
- `m x y`: Move to coordinates (start new path)
- `l x y`: Line to coordinates
- `re x y w h`: Rectangle (x, y, width, height)
- `h`: Close path

**Stroke and Fill**
- `S`: Stroke path (draw outline)
- `f` or `f*`: Fill path (fill interior)
- `B`: Fill and stroke path
- `n`: End path without stroke or fill

**Color Operators**
- `rg r g b`: Set fill color (RGB, 0-1 range)
- `RG r g b`: Set stroke color (RGB, 0-1 range)
- `g gray`: Set fill gray level
- `G gray`: Set stroke gray level

**Text Operators**
- `BT`: Begin text object
- `ET`: End text object
- `Tf font size`: Set font and size
- `Td x y`: Move text position
- `Tj (text)`: Show text string
- `TJ [array]`: Show text with individual glyph positioning
- `Tm a b c d e f`: Set text matrix for positioning

### Implementation Approach

**Content Generation**
```rust
let content = Content {
    operations: vec![
        // Draw rectangle border
        Operation::new("re", vec![x.into(), y.into(), width.into(), height.into()]),
        Operation::new("S", vec![]),  // Stroke
        
        // Draw text
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec!["F1".into(), 12.into()]),
        Operation::new("Td", vec![x.into(), y.into()]),
        Operation::new("Tj", vec![Object::string_literal("Text")]),
        Operation::new("ET", vec![]),
    ]
};
```

**Adding Content to Pages**
```rust
let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));
doc.add_page_contents(page_id, content_id);
```

### Key Insights
1. PDF coordinate system starts at bottom-left (Y=0 at bottom)
2. Text requires font resources to be added to the document
3. Operations are executed in sequence - order matters
4. Paths can be stroked, filled, or both
5. Colors are set in graphics state and persist until changed
6. Content streams must be encoded before adding to document
- Also use the tracing library for library logs, be intentful and not excessive with your log statement placement