//! Comment-preserving edits of JSONC settings text.
//!
//! Only the bytes of the edited value (or one inserted member) change; every
//! comment and all other formatting stay as written.

/// Sets the value at `path` (object keys from the top-level object) to `raw_json`.
/// Missing objects along the path are created. Returns `None` when the text is
/// not an object (for example a syntax error) so the caller never rewrites it blindly.
pub fn update_value(text: &str, path: &[&str], raw_json: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    if text.trim().is_empty() {
        return Some(format!("{}\n", nested(path, raw_json, 1)));
    }
    let start = skip_trivia(text, 0);
    if text.as_bytes().get(start) != Some(&b'{') {
        return None;
    }
    edit_object(text, start, path, raw_json, 1)
}

fn nested(path: &[&str], raw: &str, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    match path {
        [] => raw.to_string(),
        [last] => format!(
            "{{\n{pad}{}: {raw}\n{}}}",
            quote(last),
            "  ".repeat(depth - 1)
        ),
        [head, rest @ ..] => format!(
            "{{\n{pad}{}: {}\n{}}}",
            quote(head),
            nested(rest, raw, depth + 1),
            "  ".repeat(depth - 1)
        ),
    }
}

fn quote(key: &str) -> String {
    serde_json::to_string(key).unwrap_or_else(|_| format!("\"{key}\""))
}

struct Member {
    key: String,
    value: (usize, usize),
}

fn edit_object(text: &str, open: usize, path: &[&str], raw: &str, depth: usize) -> Option<String> {
    let members = members(text, open)?;
    let pad = "  ".repeat(depth);
    let Some(m) = members.iter().find(|m| m.key == path[0]) else {
        let insert = if path.len() == 1 {
            format!("{}: {raw}", quote(path[0]))
        } else {
            format!("{}: {}", quote(path[0]), nested(&path[1..], raw, depth + 1))
        };
        let comma = if members.is_empty() { "" } else { "," };
        let mut out = String::with_capacity(text.len() + insert.len() + 8);
        out.push_str(&text[..=open]);
        out.push_str(&format!("\n{pad}{insert}{comma}"));
        out.push_str(&text[open + 1..]);
        return Some(out);
    };
    let (vs, ve) = m.value;
    let replacement = if path.len() == 1 {
        raw.to_string()
    } else if text.as_bytes()[vs] == b'{' {
        let edited = edit_object(text, vs, &path[1..], raw, depth + 1)?;
        // `edited` is the whole text; splice only the object part back.
        let tail = text.len() - ve;
        edited[vs..edited.len() - tail].to_string()
    } else {
        nested(&path[1..], raw, depth + 1)
    };
    Some(format!("{}{replacement}{}", &text[..vs], &text[ve..]))
}

/// Parses the members of the object whose `{` is at `open`.
fn members(text: &str, open: usize) -> Option<Vec<Member>> {
    let b = text.as_bytes();
    let mut pos = open + 1;
    let mut out = Vec::new();
    loop {
        pos = skip_trivia(text, pos);
        match b.get(pos)? {
            b'}' => return Some(out),
            b',' => pos += 1,
            b'"' => {
                let key_end = string_end(text, pos)?;
                let key: String = serde_json::from_str(&text[pos..key_end]).ok()?;
                pos = skip_trivia(text, key_end);
                if b.get(pos) != Some(&b':') {
                    return None;
                }
                let vs = skip_trivia(text, pos + 1);
                let ve = value_end(text, vs)?;
                out.push(Member {
                    key,
                    value: (vs, ve),
                });
                pos = ve;
            }
            _ => return None,
        }
    }
}

fn skip_trivia(text: &str, mut pos: usize) -> usize {
    let b = text.as_bytes();
    loop {
        match b.get(pos) {
            Some(c) if c.is_ascii_whitespace() => pos += 1,
            Some(b'/') if b.get(pos + 1) == Some(&b'/') => {
                while b.get(pos).is_some_and(|c| *c != b'\n') {
                    pos += 1;
                }
            }
            Some(b'/') if b.get(pos + 1) == Some(&b'*') => {
                pos += 2;
                while pos < b.len() && !(b[pos] == b'*' && b.get(pos + 1) == Some(&b'/')) {
                    pos += 1;
                }
                pos = (pos + 2).min(b.len());
            }
            _ => return pos,
        }
    }
}

/// Index one past the closing quote of the string starting at `pos`.
fn string_end(text: &str, pos: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = pos + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Index one past the value starting at `pos`.
fn value_end(text: &str, pos: usize) -> Option<usize> {
    let b = text.as_bytes();
    match b.get(pos)? {
        b'"' => string_end(text, pos),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut i = pos;
            while i < b.len() {
                match b[i] {
                    b'"' => i = string_end(text, i)?,
                    b'/' if matches!(b.get(i + 1), Some(b'/' | b'*')) => i = skip_trivia(text, i),
                    b'{' | b'[' => {
                        depth += 1;
                        i += 1;
                    }
                    b'}' | b']' => {
                        depth -= 1;
                        i += 1;
                        if depth == 0 {
                            return Some(i);
                        }
                    }
                    _ => i += 1,
                }
            }
            None
        }
        _ => {
            let mut i = pos;
            while i < b.len()
                && !matches!(b[i], b',' | b'}' | b']' | b'/')
                && !b[i].is_ascii_whitespace()
            {
                i += 1;
            }
            Some(i)
        }
    }
}
