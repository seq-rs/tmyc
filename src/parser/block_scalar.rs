use std::borrow::Cow;

use crate::{Parser, Result, Value, parser::line_end};

#[derive(Clone, Copy)]
enum Chomp {
    Strip,
    Clip,
    Keep,
}

#[derive(Clone, Copy)]
enum Style {
    /// |
    Literal,
    /// >
    Folded,
}

impl<'a> Parser<'a> {
    /// Parse a literal block scalar (`|`, `|-`, `|+`)
    ///
    /// Input:
    /// ```yaml
    /// key: |
    ///   line one
    ///   line two
    /// ```
    ///
    /// Output (called on the value): `Value::String(Cow::Owned("line one\nline two\n"))`
    ///
    /// `parent_indent` is the indent of the line containing `|`. Content
    /// indent is auto-detected from the first non-blank line (or set
    /// explicitly via `|N` where N is 1-9). Chomping: `-` strip all trailing
    /// newlines, `+` keep all, default (clip) keeps one.
    pub(super) fn parse_block_scalar(&mut self) -> Result<Value<'a>> {
        let style = match self.peek() {
            Some(b'|') => Style::Literal,
            Some(b'>') => Style::Folded,
            _ => unreachable!("dispatch routes only | or > here"),
        };

        let owning_line_indent = self.current_line_indent();
        self.advance(); // consume '|' or '>'

        let chomp = match self.peek() {
            Some(b'-') => {
                self.advance();
                Chomp::Strip
            }
            Some(b'+') => {
                self.advance();
                Chomp::Keep
            }
            _ => Chomp::Clip,
        };

        // Optional explicit indent indicator between 1 and 9
        let explicit_indent = match self.peek() {
            Some(d @ b'1'..=b'9') => {
                self.advance();
                Some((d - b'0') as usize)
            }
            _ => None,
        };

        // header tail: optional comment, then mandatory linebreak;
        self.skip_spaces();
        if self.peek() == Some(b'#') {
            while let Some(b) = self.peek() {
                if line_end(b) {
                    break;
                }
                self.advance();
            }
        }

        self.consume_one_line_break();

        // detect content indent (auto/explicit)
        let content_indent = match explicit_indent {
            Some(d) => owning_line_indent + d,
            None => self.detect_block_content_indent(owning_line_indent)?,
        };

        // accumulate lines + pending blanks
        let mut buf = String::new();
        let mut pending_blanks: usize = 0;
        let mut first_content = true;
        // tracking for folding loop
        let mut prev_was_more_indented = false;

        loop {
            if self.at_eof() {
                break;
            }

            let leading = self.current_indent()?;
            let line_blank = self.is_blank_at(leading);

            if line_blank {
                if !first_content {
                    pending_blanks += 1;
                }

                self.skip_to_next_line();
                continue;
            }

            if leading < content_indent {
                break;
            }

            if !first_content {
                let this_is_more_indented = leading > content_indent;

                match style {
                    Style::Literal => {
                        buf.push('\n');
                        for _ in 0..pending_blanks {
                            buf.push('\n');
                        }
                    }
                    Style::Folded => {
                        if pending_blanks > 0 {
                            // each blank line emits a `\n`. the implicit break that would
                            // have been a space is consumed by the first blank
                            for _ in 0..pending_blanks {
                                buf.push('\n');
                            }
                        } else if prev_was_more_indented || this_is_more_indented {
                            // more indented lines preserve breaks as literal \n
                            buf.push('\n');
                        } else {
                            buf.push(' ');
                        }
                    }
                }
            }
            pending_blanks = 0;
            first_content = false;

            for _ in 0..content_indent {
                self.advance();
            }
            let start = self.pos;

            while let Some(b) = self.peek() {
                if line_end(b) {
                    break;
                }
                self.advance();
            }

            buf.push_str(&self.src[start..self.pos]);
            self.consume_one_line_break();

            prev_was_more_indented = leading > content_indent;
        }

        match chomp {
            Chomp::Strip => {}
            Chomp::Clip => {
                if !buf.is_empty() {
                    buf.push('\n');
                }
            }
            Chomp::Keep => {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                for _ in 0..pending_blanks {
                    buf.push('\n');
                }
            }
        }

        Ok(Value::String(Cow::Owned(buf)))
    }

    /// Leading-space count of the line the cursor is currently on
    fn current_line_indent(&self) -> usize {
        let bytes = self.src.as_bytes();
        let mut start = self.pos;
        while start > 0 && !matches!(bytes[start - 1], b'\n' | b'\r') {
            start -= 1;
        }
        let mut n = 0;
        while start + n < bytes.len() && bytes[start + n] == b' ' {
            n += 1;
        }
        n
    }

    /// Peek-ahead to determine block content indent
    ///
    /// Skips blank lines and returns the indent of the first non-blank line
    /// (or `parent_indent + 1` on EOF). Cursor is restored — the main loop
    /// re-scans these lines.
    fn detect_block_content_indent(&mut self, owning_line_indent: usize) -> Result<usize> {
        // peek-ahead: skip blank lines, return indent of first non-blank
        // Cursor is not advanced, we re-scan in main loop
        let saved = (self.pos, self.line, self.col);
        let min_required = owning_line_indent + 1;
        let indent = loop {
            if self.at_eof() {
                break min_required;
            }
            let leading = self.current_indent()?;
            if self.is_blank_at(leading) {
                self.skip_to_next_line();
                continue;
            }
            if leading < min_required {
                break min_required;
            }
            break leading;
        };
        self.pos = saved.0;
        self.line = saved.1;
        self.col = saved.2;

        Ok(indent)
    }

    /// True if the byte at `pos + leading` is end-of-line or EOF
    fn is_blank_at(&self, leading: usize) -> bool {
        matches!(self.peek_at(self.pos + leading), None | Some(b'\n' | b'\r'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a single-key map and return the value as a String.
    /// Most block-scalar tests live in a `k: |...` shape.
    fn parse_block(src: &str) -> String {
        let v = Parser::new(src).parse_node(0).unwrap();
        match v {
            Value::Map(pairs) => match &pairs[0].1 {
                Value::String(s) => s.to_string(),
                other => panic!("expected String value, got {other:?}"),
            },
            other => panic!("expected Map, got {other:?}"),
        }
    }

    #[test]
    fn clip_default_keeps_one_newline() {
        assert_eq!(parse_block("k: |\n  hello\n"), "hello\n");
    }

    #[test]
    fn strip_removes_trailing_newline() {
        assert_eq!(parse_block("k: |-\n  hello\n"), "hello");
    }

    #[test]
    fn keep_retains_trailing_blanks() {
        assert_eq!(parse_block("k: |+\n  hello\n\n"), "hello\n\n");
    }

    #[test]
    fn two_lines_joined_with_newline() {
        assert_eq!(parse_block("k: |\n  a\n  b\n"), "a\nb\n");
    }

    #[test]
    fn interior_blank_preserved() {
        assert_eq!(parse_block("k: |\n  a\n\n  b\n"), "a\n\nb\n");
    }

    #[test]
    fn block_ends_on_dedent() {
        let src = "k: |\n  a\n  b\nnext: v\n";
        let v = Parser::new(src).parse_node(0).unwrap();
        match v {
            Value::Map(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert!(matches!(&pairs[0].1, Value::String(s) if s == "a\nb\n"));
                assert!(matches!(&pairs[1].0, Value::String(s) if s == "next"));
                assert!(matches!(&pairs[1].1, Value::String(s) if s == "v"));
            }
            other => panic!("expected Map, got {other:?}"),
        }
    }

    #[test]
    fn explicit_indent_indicator() {
        // |2 forces content indent = parent (0) + 2 = 2;
        // "    indented" has 4 leading spaces — keeps 2; "  short" keeps 0
        assert_eq!(
            parse_block("k: |2\n    indented line\n  short line\n"),
            "  indented line\nshort line\n",
        );
    }

    #[test]
    fn eof_mid_block_no_trailing_newline() {
        assert_eq!(parse_block("k: |\n  hello"), "hello\n");
    }

    #[test]
    fn empty_block_strip() {
        let src = "k: |-\nnext: v\n";
        let v = Parser::new(src).parse_node(0).unwrap();
        match v {
            Value::Map(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert!(matches!(&pairs[0].1, Value::String(s) if s.is_empty()));
            }
            other => panic!("expected Map, got {other:?}"),
        }
    }

    #[test]
    fn pem_blob_shape() {
        let src = "\
data: |
  -----BEGIN CERTIFICATE-----
  MIIDazCCAlOgAwIBAgI=
  -----END CERTIFICATE-----
";
        assert_eq!(
            parse_block(src),
            "-----BEGIN CERTIFICATE-----\nMIIDazCCAlOgAwIBAgI=\n-----END CERTIFICATE-----\n",
        );
    }

    #[test]
    fn header_comment_ok() {
        assert_eq!(parse_block("k: | # header comment\n  hi\n"), "hi\n");
    }

    #[test]
    fn tab_in_indent_errors() {
        let src = "k: |\n\thello\n";
        let result = Parser::new(src).parse_node(0);
        assert!(result.is_err(), "expected tab-in-indent to error");
    }

    // folded scalars (>, >-, >+)

    #[test]
    fn folded_simple() {
        assert_eq!(parse_block("k: >\n  a\n  b\n"), "a b\n");
    }

    #[test]
    fn folded_strip() {
        assert_eq!(parse_block("k: >-\n  a\n  b\n"), "a b");
    }

    #[test]
    fn folded_keep() {
        assert_eq!(parse_block("k: >+\n  a\n\n"), "a\n\n");
    }

    #[test]
    fn folded_blank_line_separates() {
        assert_eq!(parse_block("k: >\n  a\n\n  b\n"), "a\nb\n");
    }

    #[test]
    fn folded_two_blank_lines() {
        assert_eq!(parse_block("k: >\n  a\n\n\n  b\n"), "a\n\nb\n");
    }

    #[test]
    fn folded_more_indented_preserves_break() {
        assert_eq!(
            parse_block("k: >\n  a\n    code\n  b\n"),
            "a\n  code\nb\n",
        );
    }

    #[test]
    fn folded_more_indented_run() {
        // multiple consecutive more-indented lines keep all their breaks
        assert_eq!(
            parse_block("k: >\n  a\n    one\n    two\n  b\n"),
            "a\n  one\n  two\nb\n",
        );
    }

    #[test]
    fn folded_explicit_indent_indicator() {
        // >2 forces content_indent = 0 + 2 = 2.
        // "    foo" (leading 4) is more-indented (4 > 2) → break preserved as \n
        // and 2 leading spaces survive in output. "  bar" (leading 2) is at
        // content_indent → 0 spaces survive.
        assert_eq!(
            parse_block("k: >2\n    foo\n  bar\n"),
            "  foo\nbar\n",
        );
    }

    #[test]
    fn folded_eof_no_trailing_newline() {
        assert_eq!(parse_block("k: >\n  hello"), "hello\n");
    }

    #[test]
    fn folded_empty_block_strip() {
        let src = "k: >-\nnext: v\n";
        let v = Parser::new(src).parse_node(0).unwrap();
        match v {
            Value::Map(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert!(matches!(&pairs[0].1, Value::String(s) if s.is_empty()));
            }
            other => panic!("expected Map, got {other:?}"),
        }
    }

    #[test]
    fn folded_paragraph_shape() {
        let src = "k: >\n  Lorem ipsum\n  dolor sit amet.\n\n  Consectetur adipiscing\n  elit.\n";
        assert_eq!(
            parse_block(src),
            "Lorem ipsum dolor sit amet.\nConsectetur adipiscing elit.\n",
        );
    }
}
