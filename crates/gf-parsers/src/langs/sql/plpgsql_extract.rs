//! Lossless `$tag$...$tag$` body extractor for `PostgreSQL` `LANGUAGE plpgsql` routines.

/// Extracted `LANGUAGE plpgsql` routine header and `$tag$...$tag$` body with exact 1-based lines.
pub(super) struct PlpgsqlBody {
    pub(super) name: String,
    pub(super) header: String,
    pub(super) header_line: u32,
    pub(super) body: String,
    pub(super) body_start_line: u32,
    pub(super) end_line: u32,
}

fn find_dollar_tag(s: &str) -> Option<(usize, &str)> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] != b'$' {
            continue;
        }
        let end =
            (i + 1..bytes.len()).find(|&j| !(bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_'));
        if let Some(j) = end
            && bytes[j] == b'$'
        {
            return Some((i, &s[i..=j]));
        }
    }
    None
}

fn line_at_offset(source: &str, byte_offset: usize) -> u32 {
    let prefix = source.get(..byte_offset).unwrap_or(source);
    u32::try_from(prefix.bytes().filter(|&b| b == b'\n').count() + 1).unwrap_or(u32::MAX)
}

fn extract_routine_name(header: &str) -> String {
    let upper = header.to_ascii_uppercase();
    for kw in ["FUNCTION ", "PROCEDURE "] {
        if let Some(pos) = upper.find(kw) {
            let name: String = header[pos + kw.len()..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '"'))
                .collect();
            if !name.is_empty() {
                return name;
            }
        }
    }
    String::from("<anonymous>")
}

pub(super) fn extract_plpgsql_bodies(source: &str) -> Vec<PlpgsqlBody> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some((rel_open, tag)) = find_dollar_tag(&source[cursor..]) {
        let open_abs = cursor + rel_open;
        let body_start = open_abs + tag.len();
        let Some(rel_close) = source[body_start..].find(tag) else {
            break;
        };
        let close_abs = body_start + rel_close;
        let after_end = (close_abs + tag.len() + 120).min(source.len());
        let before_start = source[..open_abs]
            .to_ascii_uppercase()
            .rfind("CREATE")
            .or_else(|| source[..open_abs].to_ascii_uppercase().rfind("DO "))
            .unwrap_or(cursor);
        let header = &source[before_start..open_abs];
        let trailer = &source[close_abs + tag.len()..after_end];
        if header.to_ascii_lowercase().contains("plpgsql")
            || trailer.to_ascii_lowercase().contains("plpgsql")
        {
            out.push(PlpgsqlBody {
                name: extract_routine_name(header),
                header: String::from(header),
                header_line: line_at_offset(source, before_start),
                body: String::from(&source[body_start..close_abs]),
                body_start_line: line_at_offset(source, body_start),
                end_line: line_at_offset(source, close_abs),
            });
        }
        cursor = close_abs + tag.len();
    }
    out
}
