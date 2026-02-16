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
            let word_length = word.chars().count();

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
                // Break the word using char_indices to avoid splitting multi-byte characters
                let mut remaining = word;
                while remaining.chars().count() > max_chars_per_line {
                    let split_byte = remaining
                        .char_indices()
                        .nth(max_chars_per_line)
                        .map(|(idx, _)| idx)
                        .unwrap_or(remaining.len());
                    let (chunk, rest) = remaining.split_at(split_byte);
                    all_lines.push(chunk.to_string());
                    remaining = rest;
                }
                if !remaining.is_empty() {
                    current_line = remaining.to_string();
                    current_length = remaining.chars().count();
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

/// Break text into lines using actual font metrics for width measurement.
///
/// Same logic as `wrap_text` but uses `FontMetrics::text_width()` for accurate
/// measurement instead of the fixed `DEFAULT_CHAR_WIDTH_RATIO` heuristic.
pub fn wrap_text_with_metrics(
    text: &str,
    max_width: f32,
    font_size: f32,
    metrics: &dyn crate::font::FontMetrics,
) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut all_lines = Vec::new();

    for segment in text.split('\n') {
        if segment.is_empty() {
            all_lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = segment.split_whitespace().collect();

        if words.is_empty() {
            all_lines.push(String::new());
            continue;
        }

        let space_width = metrics.char_width(' ', font_size);
        let mut current_line = String::new();
        let mut current_width: f32 = 0.0;

        for word in words {
            let word_width = metrics.text_width(word, font_size);

            if current_width > 0.0 && current_width + space_width + word_width > max_width {
                all_lines.push(current_line.trim().to_string());
                current_line = word.to_string();
                current_width = word_width;
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                    current_width += space_width;
                }
                current_line.push_str(word);
                current_width += word_width;
            }

            // Handle very long words that exceed max_width
            if word_width > max_width {
                let mut remaining = word;
                while !remaining.is_empty() {
                    let mut split_at_char = 0;
                    let mut accumulated = 0.0;
                    for (i, ch) in remaining.char_indices() {
                        let cw = metrics.char_width(ch, font_size);
                        if accumulated + cw > max_width && split_at_char > 0 {
                            break;
                        }
                        accumulated += cw;
                        split_at_char = i + ch.len_utf8();
                    }
                    if split_at_char == 0 {
                        // Single char wider than max_width, take at least one char
                        split_at_char = remaining
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| i)
                            .unwrap_or(remaining.len());
                    }
                    let (chunk, rest) = remaining.split_at(split_at_char);
                    if rest.is_empty() {
                        current_line = chunk.to_string();
                        current_width = metrics.text_width(chunk, font_size);
                    } else {
                        all_lines.push(chunk.to_string());
                    }
                    remaining = rest;
                }
            }
        }

        if !current_line.trim().is_empty() {
            all_lines.push(current_line.trim().to_string());
        }
    }

    if all_lines.is_empty() {
        all_lines.push(String::new());
    }

    trace!("Wrapped text (with metrics) into {} lines", all_lines.len());
    all_lines
}

/// Calculate the height needed for wrapped text using font metrics
pub fn calculate_wrapped_text_height_with_metrics(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_spacing: f32,
    metrics: &dyn crate::font::FontMetrics,
) -> f32 {
    let lines = wrap_text_with_metrics(text, max_width, font_size, metrics);
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

    #[test]
    fn test_multibyte_char_wrapping_no_panic() {
        // This should not panic even with multi-byte UTF-8 characters
        let text =
            "\u{00e9}\u{00e9}\u{00e9}\u{00e9}\u{00e9}\u{00e9}\u{00e9}\u{00e9}\u{00e9}\u{00e9}";
        let lines = wrap_text(text, 20.0, 10.0);
        assert!(!lines.is_empty());
        // Verify all chars are preserved across lines
        let total_chars: usize = lines.iter().map(|l| l.chars().count()).sum();
        assert_eq!(total_chars, 10);
    }

    #[test]
    fn test_multibyte_long_word_splitting() {
        // A long word of multi-byte characters that exceeds line width
        let text = "caf\u{00e9}caf\u{00e9}caf\u{00e9}caf\u{00e9}caf\u{00e9}";
        let lines = wrap_text(text, 30.0, 10.0);
        // Should split without panicking
        assert!(!lines.is_empty());
        // Verify all chars preserved
        let total: String = lines.join("");
        assert_eq!(total, text);
    }

    #[test]
    fn test_char_count_vs_byte_count() {
        // "\u{00e9}" is 2 bytes in UTF-8 but 1 character
        // With char counting, 4 chars should fit ~4 char widths
        let text = "\u{00e9}\u{00e9}\u{00e9}\u{00e9}";
        assert_eq!(text.len(), 8); // 8 bytes
        assert_eq!(text.chars().count(), 4); // 4 characters

        // With font_size=10 and ratio=0.5, char_width=5, max_chars_per_line=20/5=4
        let lines = wrap_text(text, 20.0, 10.0);
        // All 4 chars should fit on one line
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], text);
    }

    #[test]
    fn test_cjk_characters_wrapping() {
        // CJK characters are 3 bytes each in UTF-8
        let text = "\u{4f60}\u{597d}\u{4e16}\u{754c}"; // "hello world" in Chinese
        assert_eq!(text.len(), 12); // 12 bytes
        assert_eq!(text.chars().count(), 4); // 4 characters

        let lines = wrap_text(text, 15.0, 10.0);
        // Should split correctly by character count, not byte count
        assert!(!lines.is_empty());
        let total: String = lines.join("");
        assert_eq!(total, text);
    }
}
