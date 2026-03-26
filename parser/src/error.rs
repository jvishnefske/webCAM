use std::fmt;

/// A parse error with location information.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub expected: Vec<String>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse error at line {}:{}: expected {}",
            self.line,
            self.column,
            self.expected.join(" or ")
        )
    }
}

impl std::error::Error for ParseError {}

/// Convert a byte offset in `source` to (line, column), both 1-based.
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_to_line_col_first_char() {
        assert_eq!(offset_to_line_col("hello", 0), (1, 1));
    }

    #[test]
    fn offset_to_line_col_second_line() {
        assert_eq!(offset_to_line_col("ab\ncd", 3), (2, 1));
        assert_eq!(offset_to_line_col("ab\ncd", 4), (2, 2));
    }

    #[test]
    fn parse_error_display() {
        let e = ParseError {
            line: 3,
            column: 5,
            expected: vec!["\"block\"".into(), "identifier".into()],
        };
        assert_eq!(e.to_string(), "parse error at line 3:5: expected \"block\" or identifier");
    }
}
