//! Lossless multi-block SQL segmenter preserving exact 1-based line offsets across
//! MariaDB `DELIMITER` and Oracle SQL*Plus `/` terminators (including `q'[...]'` literals).

/// Single logical SQL/PLSQL block with its exact 1-based starting line in the original file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SqlSegment {
    /// Raw source text of the segment.
    pub text: String,
    /// 1-based starting line number in the original file.
    pub start_line: u32,
}

fn push_segment(out: &mut Vec<SqlSegment>, buf: &mut String, start_line: u32) {
    if buf.trim().is_empty() {
        buf.clear();
    } else {
        out.push(SqlSegment {
            text: core::mem::take(buf),
            start_line,
        });
    }
}

struct DelimState {
    delim: String,
    buf: String,
    start: Option<u32>,
}

impl DelimState {
    fn flush(&mut self, out: &mut Vec<SqlSegment>) {
        if let Some(s) = self.start.take() {
            push_segment(out, &mut self.buf, s);
        }
    }

    fn step_line(&mut self, line: &str, ln: u32, out: &mut Vec<SqlSegment>) {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if let Some(rest) = upper.strip_prefix("DELIMITER ") {
            self.flush(out);
            let tok = trimmed[trimmed.len() - rest.len()..].trim();
            if !tok.is_empty() {
                self.delim = String::from(tok);
            }
            return;
        }
        if self.start.is_none() && !trimmed.is_empty() {
            self.start = Some(ln);
        }
        if self.delim != ";" && trimmed.ends_with(self.delim.as_str()) {
            let cut = line.rfind(self.delim.as_str()).unwrap_or(line.len());
            self.buf.push_str(&line[..cut]);
            self.buf.push('\n');
            self.flush(out);
            return;
        }
        self.buf.push_str(line);
        self.buf.push('\n');
    }
}

/// Splits MariaDB scripts honoring client `DELIMITER <tok>` directives with exact 1-based lines.
#[must_use]
pub fn segment_mariadb_delimiters(source: &str) -> Vec<SqlSegment> {
    let mut out = Vec::new();
    let mut st = DelimState {
        delim: String::from(";"),
        buf: String::new(),
        start: None,
    };
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        st.step_line(line, ln, &mut out);
    }
    if let Some(s) = st.start {
        push_segment(&mut out, &mut st.buf, s);
    }
    out
}

const fn oracle_q_closing(open: char) -> char {
    match open {
        '[' => ']',
        '{' => '}',
        '<' => '>',
        '(' => ')',
        other => other,
    }
}

fn is_sqlplus_client_directive(trimmed_upper: &str) -> bool {
    ["SET ", "WHENEVER ", "SHOW ", "PROMPT "]
        .iter()
        .any(|p| trimmed_upper.starts_with(p))
        || matches!(trimmed_upper, "SHOW ERRORS" | "EXIT" | "QUIT")
}

struct QuoteScan {
    in_block_comment: bool,
    in_single: bool,
    q_close: Option<char>,
}

impl QuoteScan {
    const fn new() -> Self {
        Self {
            in_block_comment: false,
            in_single: false,
            q_close: None,
        }
    }

    const fn is_code_level(&self) -> bool {
        !self.in_block_comment && !self.in_single && self.q_close.is_none()
    }

    fn step_quoted(&mut self, bytes: &[u8], i: usize) -> usize {
        if self.in_block_comment {
            let closed = bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/');
            self.in_block_comment = !closed;
            return i + 1 + usize::from(closed);
        }
        if let Some(qc) = self.q_close {
            let closed = bytes[i] == qc as u8 && bytes.get(i + 1) == Some(&b'\'');
            if closed {
                self.q_close = None;
            }
            return i + 1 + usize::from(closed);
        }
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                return i + 2;
            }
            self.in_single = false;
        }
        i + 1
    }

    fn advance_code_byte(&mut self, bytes: &[u8], i: usize) -> usize {
        if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            return bytes.len();
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            self.in_block_comment = true;
            return i + 2;
        }
        if (bytes[i] == b'q' || bytes[i] == b'Q')
            && bytes.get(i + 1) == Some(&b'\'')
            && let Some(&open) = bytes.get(i + 2)
        {
            self.q_close = Some(oracle_q_closing(open as char));
            return i + 3;
        }
        if bytes[i] == b'\'' {
            self.in_single = true;
        }
        i + 1
    }

    fn advance_line(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            i = if self.is_code_level() {
                self.advance_code_byte(bytes, i)
            } else {
                self.step_quoted(bytes, i)
            };
        }
    }
}

struct OracleSegState {
    scan: QuoteScan,
    buf: String,
    start: Option<u32>,
}

impl OracleSegState {
    fn step_line(&mut self, line: &str, ln: u32, out: &mut Vec<SqlSegment>) {
        let trimmed = line.trim();
        if self.scan.is_code_level() && trimmed == "/" {
            if let Some(s) = self.start.take() {
                push_segment(out, &mut self.buf, s);
            }
            return;
        }
        if self.scan.is_code_level()
            && self.start.is_none()
            && is_sqlplus_client_directive(&trimmed.to_ascii_uppercase())
        {
            return;
        }
        if self.start.is_none() && !trimmed.is_empty() {
            self.start = Some(ln);
        }
        self.scan.advance_line(line);
        self.buf.push_str(line);
        self.buf.push('\n');
    }
}

/// Splits Oracle SQL*Plus scripts on standalone `/` lines while preserving comments, `'...'`, and `q'[...]'`.
#[must_use]
pub fn segment_oracle_slash_units(source: &str) -> Vec<SqlSegment> {
    let mut out = Vec::new();
    let mut st = OracleSegState {
        scan: QuoteScan::new(),
        buf: String::new(),
        start: None,
    };
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        st.step_line(line, ln, &mut out);
    }
    if let Some(s) = st.start {
        push_segment(&mut out, &mut st.buf, s);
    }
    out
}
