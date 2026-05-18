use crate::{Parser, Result};

impl<'a> Parser<'a> {

    pub(super) fn decode_double_quoted_escape(
        &mut self,
        out: &mut String,
        _parent_indent: usize,
    ) -> Result<()> {
        let c = self.peek().ok_or_else(|| self.err("escape at EOF"))?;
        match c {
            b'0' => {
                out.push('\0');
                self.advance();
            }
            b'a' => {
                out.push('\x07');
                self.advance();
            }
            b'b' => {
                out.push('\x08');
                self.advance();
            }
            b't' => {
                out.push('\t');
                self.advance();
            }
            b'n' => {
                out.push('\n');
                self.advance();
            }
            b'v' => {
                out.push('\x0B');
                self.advance();
            }
            b'f' => {
                out.push('\x0C');
                self.advance();
            }
            b'r' => {
                out.push('\r');
                self.advance();
            }
            b'e' => {
                out.push('\x1B');
                self.advance();
            }
            b' ' => {
                out.push(' ');
                self.advance();
            }
            b'"' => {
                out.push('"');
                self.advance();
            }
            b'/' => {
                out.push('/');
                self.advance();
            }
            b'\\' => {
                out.push('\\');
                self.advance();
            }
            b'N' => {
                out.push('\u{85}');
                self.advance();
            }
            b'_' => {
                out.push('\u{A0}');
                self.advance();
            }
            b'L' => {
                out.push('\u{2028}');
                self.advance();
            }
            b'P' => {
                out.push('\u{2029}');
                self.advance();
            }
            b'x' => {
                self.advance();
                self.parse_hex_escape(2, out)?;
            }
            b'u' => {
                self.advance();
                self.parse_hex_escape(4, out)?;
            }
            b'U' => {
                self.advance();
                self.parse_hex_escape(8, out)?;
            }
            b'\n' | b'\r' => {
                self.consume_one_line_break();
                while matches!(self.peek(), Some(b' ' | b'\t')) {
                    self.advance();
                }
                // no push
            }
            other => return Err(self.err(format!("unknown escape \\{}", other as char))),
        }
        Ok(())
    }

    pub(super) fn parse_hex_escape(&mut self, n: usize, out: &mut String) -> Result<()> {
        let start = self.pos;

        for _ in 0..n {
            let b = self
                .peek()
                .ok_or_else(|| self.err("truncated hex escape"))?;
            if !b.is_ascii_hexdigit() {
                return Err(self.err(format!(
                    "invalid hex digit in \\{}{}",
                    n, b as char
                )));
            }
            self.advance();
        }
        let v = u32::from_str_radix(&self.src[start..self.pos], 16)
            .map_err(|_| self.err("invalid hex"))?;
        let c = char::from_u32(v).ok_or_else(|| self.err("invalid unicode codepoint"))?;
        out.push(c);
        Ok(())
    }
}
