//! Unicode table example using TrueType font embedding with ttf-parser
//!
//! This example demonstrates how to:
//! 1. Load a system TTF font
//! 2. Embed it in a PDF as a Type0/CIDFontType2 font
//! 3. Use TtfFontMetrics for accurate text measurement
//! 4. Render Unicode text (accented Latin, symbols) in table cells

use lopdf::{Document, Object, Stream, dictionary};
use lopdf_table::{Cell, Row, Table, TableDrawing, TableStyle, TtfFontMetrics};

/// Try to load a system TrueType font, returning (font_data, font_name).
fn load_system_font() -> Option<(Vec<u8>, &'static str)> {
    let candidates = [
        ("/System/Library/Fonts/Helvetica.ttc", "Helvetica"),
        ("/System/Library/Fonts/Supplemental/Arial.ttf", "Arial"),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "DejaVuSans",
        ),
        ("C:\\Windows\\Fonts\\arial.ttf", "Arial"),
    ];
    for (path, name) in &candidates {
        if let Ok(data) = std::fs::read(path) {
            return Some((data, name));
        }
    }
    None
}

/// Build the PDF font objects needed for a Type0 CIDFontType2 embedding.
///
/// This is the caller's responsibility - lopdf-table only does measurement
/// and glyph ID encoding; the caller must set up the font dictionaries.
fn embed_ttf_font(doc: &mut Document, font_data: &[u8], base_font_name: &str) -> lopdf::ObjectId {
    let face = ttf_parser::Face::parse(font_data, 0).expect("valid font");
    let units_per_em = face.units_per_em();

    // Font descriptor
    let font_descriptor_id = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => base_font_name,
        "Flags" => 32, // Nonsymbolic
        "ItalicAngle" => 0,
        "Ascent" => face.ascender() as i64,
        "Descent" => face.descender() as i64,
        "CapHeight" => face.capital_height().unwrap_or(face.ascender()) as i64,
        "StemV" => 80,
        "FontBBox" => vec![
            Object::Integer(0),
            Object::Integer(face.descender() as i64),
            Object::Integer(units_per_em as i64),
            Object::Integer(face.ascender() as i64),
        ],
    });

    // Embed font data as stream
    let font_stream = Stream::new(
        dictionary! {
            "Length1" => font_data.len() as i64,
        },
        font_data.to_vec(),
    );
    let font_stream_id = doc.add_object(font_stream);

    // Update descriptor to reference the embedded font
    if let Ok(Object::Dictionary(desc)) = doc.get_object_mut(font_descriptor_id) {
        desc.set("FontFile2", font_stream_id);
    }

    // CIDFont dictionary
    let cid_font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => base_font_name,
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => font_descriptor_id,
        "DW" => 1000, // Default glyph width
    });

    // Type0 (composite) font
    doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => base_font_name,
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![cid_font_id.into()],
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load a system font
    let (font_data, font_name) =
        load_system_font().expect("Could not find a system TrueType font to use for this example");

    println!("Using system font: {font_name}");

    // 2. Create font metrics from the raw font data
    let metrics = TtfFontMetrics::new(font_data.clone())?;

    // 3. Build the PDF document
    let mut doc = Document::with_version("1.5");

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

    if let Ok(Object::Dictionary(pages)) = doc.get_object_mut(pages_id) {
        if let Ok(Object::Array(kids)) = pages.get_mut(b"Kids") {
            kids.push(page_id.into());
        }
        pages.set("Count", Object::Integer(1));
    }

    // 4. Embed the font into the PDF (caller's responsibility)
    let embedded_font_id = embed_ttf_font(&mut doc, &font_data, font_name);

    // Also add a Type1 fallback for bold headers
    let font_bold_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
        "Encoding" => "WinAnsiEncoding",
    });

    // Set up page resources with both fonts
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "EF0" => embedded_font_id,
            "F1-Bold" => font_bold_id,
        },
    });

    if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
        page.set("Resources", resources_id);
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // 5. Build a table with Unicode content
    let mut style = TableStyle::default();
    style.embedded_font_resource_name = Some("EF0".to_string());

    let table = Table::new()
        .with_style(style)
        .with_font_metrics(metrics)
        .add_row(Row::new(vec![
            Cell::new("Language").bold(),
            Cell::new("Greeting").bold(),
            Cell::new("Description").bold(),
        ]))
        .add_row(Row::new(vec![
            Cell::new("French"),
            Cell::new("Bonjour le monde!"),
            Cell::new("Accented Latin characters"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("German"),
            Cell::new("Gr\u{00fc}\u{00df}e aus Berlin"),
            Cell::new("Umlauts and sharp s"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Spanish"),
            Cell::new("\u{00a1}Hola! \u{00bf}C\u{00f3}mo est\u{00e1}s?"),
            Cell::new("Inverted punctuation"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Symbols"),
            Cell::new("\u{00a9} 2025 \u{2014} All rights reserved \u{2122}"),
            Cell::new("Copyright, em-dash, trademark"),
        ]))
        .add_row(Row::new(vec![
            Cell::new("Currency"),
            Cell::new("\u{00a3}100 \u{20ac}200 \u{00a5}300"),
            Cell::new("Pound, Euro, Yen"),
        ]))
        .with_border(1.0)
        .with_total_width(500.0);

    // 6. Draw the table
    doc.draw_table(page_id, table, (50.0, 750.0))?;

    // Save
    doc.save("unicode_table.pdf")?;
    println!("PDF saved as 'unicode_table.pdf'");

    Ok(())
}
