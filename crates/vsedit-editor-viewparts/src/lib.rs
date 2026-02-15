//! Editor visual parts (line numbers, margins, rulers).

/// Line number rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberMode { Absolute, Relative, Interval(u32) }

/// Format a line number for display.
pub fn format_line_number(line: u32, current_line: u32, mode: LineNumberMode) -> String {
    match mode {
        LineNumberMode::Absolute => format!("{}", line),
        LineNumberMode::Relative => {
            if line == current_line { format!("{}", line) }
            else { format!("{}", (line as i64 - current_line as i64).unsigned_abs()) }
        }
        LineNumberMode::Interval(n) => {
            if line == current_line || line % n == 0 { format!("{}", line) }
            else { String::new() }
        }
    }
}

/// Ruler position.
pub struct Ruler { pub column: u32 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_numbers() {
        assert_eq!(format_line_number(5, 3, LineNumberMode::Absolute), "5");
    }

    #[test]
    fn relative_numbers() {
        assert_eq!(format_line_number(5, 3, LineNumberMode::Relative), "2");
        assert_eq!(format_line_number(3, 3, LineNumberMode::Relative), "3");
    }

    #[test]
    fn interval_numbers() {
        assert_eq!(format_line_number(10, 3, LineNumberMode::Interval(5)), "10");
        assert_eq!(format_line_number(7, 3, LineNumberMode::Interval(5)), "");
        assert_eq!(format_line_number(3, 3, LineNumberMode::Interval(5)), "3");
    }
}
