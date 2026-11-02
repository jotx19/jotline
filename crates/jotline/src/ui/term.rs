//! Terminal capabilities, colors, and text layout.

use std::env;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    Accent,
    Info,
    Success,
    Warning,
    Danger,
    Muted,
}

pub fn glyph(tone: Tone) -> &'static str {
    match tone {
        Tone::Accent => "✦",
        Tone::Info => "●",
        Tone::Success => "✓",
        Tone::Warning => "▲",
        Tone::Danger => "✕",
        Tone::Muted => "·",
    }
}

pub fn supports_live_output() -> bool {
    io::stdout().is_terminal() && env::var("TERM").map(|term| term != "dumb").unwrap_or(true)
}

pub fn supports_color() -> bool {
    supports_live_output() && env::var_os("NO_COLOR").is_none()
}

pub fn paint(value: &str, tone: Tone, bold: bool) -> String {
    if !supports_color() {
        return value.to_string();
    }
    let color = match tone {
        Tone::Accent => "38;5;81",
        Tone::Info => "38;5;111",
        Tone::Success => "38;5;114",
        Tone::Warning => "38;5;214",
        Tone::Danger => "38;5;203",
        Tone::Muted => "38;5;246",
    };
    let weight = if bold { "1;" } else { "" };
    format!("\x1b[{weight}{color}m{value}\x1b[0m")
}

pub fn clear_live_line() {
    if supports_live_output() {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }
}

pub fn panel_width() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(60, 120))
        .unwrap_or(96)
}

/// Display width in terminal columns, approximated as one column per character.
pub fn display_width(value: &str) -> usize {
    value.chars().count()
}

/// Word-wraps `text` to `width` columns, preserving leading indentation and splitting words
/// longer than a line.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut wrapped = Vec::new();
    for raw in text.lines() {
        if raw.trim().is_empty() {
            wrapped.push(String::new());
            continue;
        }
        let indent_len = raw
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .count()
            .min(width / 2);
        let indent = " ".repeat(indent_len);
        let available = width - indent_len;
        let mut current = String::new();
        for word in raw.split_whitespace() {
            let word_len = word.chars().count();
            let current_len = current.chars().count();
            if current_len > 0 && current_len + 1 + word_len <= available {
                current.push(' ');
                current.push_str(word);
                continue;
            }
            if current_len > 0 {
                wrapped.push(format!("{indent}{current}"));
                current.clear();
            }
            if word_len <= available {
                current.push_str(word);
            } else {
                let chars = word.chars().collect::<Vec<_>>();
                for chunk in chars.chunks(available) {
                    let piece = chunk.iter().collect::<String>();
                    if piece.chars().count() == available {
                        wrapped.push(format!("{indent}{piece}"));
                    } else {
                        current = piece;
                    }
                }
            }
        }
        if !current.is_empty() {
            wrapped.push(format!("{indent}{current}"));
        }
    }
    if wrapped.is_empty() {
        wrapped.push(String::new());
    }
    wrapped
}

/// Shortens long values in the middle: `abcdef...uvwxyz`.
pub fn compact(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let head = (max_chars / 2).saturating_sub(1);
    let tail = max_chars.saturating_sub(head + 3);
    let start = value.chars().take(head).collect::<String>();
    let end = value.chars().skip(count - tail).collect::<String>();
    format!("{start}...{end}")
}

pub fn format_elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else if duration.as_millis() >= 1000 {
        format!("{:.1}s", duration.as_secs_f32())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_respects_width_and_indent() {
        let lines = wrap_text("  alpha beta gamma delta", 12);
        assert_eq!(lines, ["  alpha beta", "  gamma", "  delta"]);
        assert!(lines.iter().all(|line| line.chars().count() <= 12));
    }

    #[test]
    fn wrap_splits_long_words() {
        let lines = wrap_text("abcdefghijklmnopqrst", 8);
        assert_eq!(lines, ["abcdefgh", "ijklmnop", "qrst"]);
    }

    #[test]
    fn wrap_keeps_blank_lines() {
        assert_eq!(wrap_text("a\n\nb", 10), ["a", "", "b"]);
        assert_eq!(wrap_text("", 10), [""]);
    }

    #[test]
    fn compact_keeps_both_ends() {
        assert_eq!(compact("short", 10), "short");
        let out = compact("qwen2.5-coder:32b-instruct-q4_K_M", 16);
        assert_eq!(out.chars().count(), 16);
        assert!(out.starts_with("qwen2.5") && out.ends_with("_K_M"));
    }

    #[test]
    fn elapsed_formats() {
        assert_eq!(format_elapsed(Duration::from_millis(250)), "250ms");
        assert_eq!(format_elapsed(Duration::from_millis(2500)), "2.5s");
        assert_eq!(format_elapsed(Duration::from_secs(125)), "2m05s");
    }
}
