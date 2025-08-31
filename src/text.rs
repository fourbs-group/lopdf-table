//! Text handling and wrapping utilities

use crate::constants::*;
use tracing::trace;

/// Break text into lines that fit within the specified width
pub fn wrap_text(text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let char_width = font_size * DEFAULT_CHAR_WIDTH_RATIO; // Simplified character width estimation
    let max_chars_per_line = (max_width / char_width) as usize;

    if max_chars_per_line == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.is_empty() {
        return vec![text.to_string()];
    }

    let mut current_line = String::new();
    let mut current_length = 0;

    for word in words {
        let word_length = word.len();

        // Check if adding this word would exceed the line width
        if current_length > 0 && current_length + 1 + word_length > max_chars_per_line {
            // Start a new line
            lines.push(current_line.trim().to_string());
            current_line = word.to_string();
            current_length = word_length;
        } else {
            // Add word to current line
            if !current_line.is_empty() {
                current_line.push(' ');
                current_length += 1;
            }
            current_line.push_str(word);
            current_length += word_length;
        }

        // Handle very long words that don't fit on a single line
        if word_length > max_chars_per_line {
            // Break the word
            let mut remaining = word;
            while remaining.len() > max_chars_per_line {
                let (chunk, rest) = remaining.split_at(max_chars_per_line);
                lines.push(chunk.to_string());
                remaining = rest;
            }
            if !remaining.is_empty() {
                current_line = remaining.to_string();
                current_length = remaining.len();
            }
        }
    }

    // Add the last line if not empty
    if !current_line.trim().is_empty() {
        lines.push(current_line.trim().to_string());
    }

    trace!("Wrapped text into {} lines", lines.len());
    lines
}

/// Calculate the height needed for wrapped text
pub fn calculate_wrapped_text_height(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_spacing: f32,
) -> f32 {
    let lines = wrap_text(text, max_width, font_size);
    let line_height = font_size * line_spacing;
    lines.len() as f32 * line_height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text() {
        let text = "This is a long piece of text that should be wrapped into multiple lines";
        let lines = wrap_text(text, 100.0, 10.0);
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_empty_text() {
        let lines = wrap_text("", 100.0, 10.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "");
    }

    #[test]
    fn test_single_long_word() {
        let text = "supercalifragilisticexpialidocious";
        let lines = wrap_text(text, 50.0, 10.0);
        assert!(lines.len() >= 1);
    }
}
