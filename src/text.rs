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

    let mut all_lines = Vec::new();

    // First, split by explicit newlines to preserve line breaks
    let segments: Vec<&str> = text.split('\n').collect();

    for segment in segments {
        // For empty segments (from consecutive newlines), add an empty line
        if segment.is_empty() {
            all_lines.push(String::new());
            continue;
        }

        // Now wrap each segment that's too long
        let words: Vec<&str> = segment.split_whitespace().collect();

        if words.is_empty() {
            // Segment has only whitespace, preserve it as empty line
            all_lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_length = 0;

        for word in words {
            let word_length = word.len();

            // Check if adding this word would exceed the line width
            if current_length > 0 && current_length + 1 + word_length > max_chars_per_line {
                // Start a new line
                all_lines.push(current_line.trim().to_string());
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
                    all_lines.push(chunk.to_string());
                    remaining = rest;
                }
                if !remaining.is_empty() {
                    current_line = remaining.to_string();
                    current_length = remaining.len();
                }
            }
        }

        // Add the last line of this segment if not empty
        if !current_line.trim().is_empty() {
            all_lines.push(current_line.trim().to_string());
        }
    }

    // If we ended up with no lines (shouldn't happen), return at least one empty line
    if all_lines.is_empty() {
        all_lines.push(String::new());
    }

    trace!("Wrapped text into {} lines", all_lines.len());
    all_lines
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

    #[test]
    fn test_text_with_newlines() {
        let text = "Line 1\nLine 2\nLine 3";
        let lines = wrap_text(text, 200.0, 10.0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(lines[1], "Line 2");
        assert_eq!(lines[2], "Line 3");
    }

    #[test]
    fn test_text_with_multiple_newlines() {
        let text = "Line 1\n\nLine 3\n\n\nLine 6";
        let lines = wrap_text(text, 200.0, 10.0);
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "Line 3");
        assert_eq!(lines[3], "");
        assert_eq!(lines[4], "");
        assert_eq!(lines[5], "Line 6");
    }

    #[test]
    fn test_text_with_newlines_and_wrapping() {
        let text = "This is a long first line that needs wrapping\nShort line\nAnother long line that also needs to be wrapped";
        let lines = wrap_text(text, 100.0, 10.0);
        // Should have more than 3 lines due to wrapping
        assert!(lines.len() > 3);
        // Check that "Short line" is preserved as its own line
        assert!(lines.contains(&"Short line".to_string()));
    }

    #[test]
    fn test_text_with_only_newlines() {
        let text = "\n\n\n";
        let lines = wrap_text(text, 100.0, 10.0);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|line| line.is_empty()));
    }

    #[test]
    fn test_text_height_with_newlines() {
        let text = "Line 1\nLine 2\nLine 3";
        let height = calculate_wrapped_text_height(text, 200.0, 10.0, 1.2);
        // 3 lines * 10.0 font size * 1.2 line spacing = 36.0
        assert_eq!(height, 36.0);
    }
}
