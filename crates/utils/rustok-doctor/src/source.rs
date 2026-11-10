//! Lexical views of Rust source used by the hygiene rules.

/// Code keeps literals and masks comments. Syntax also masks literals, so delimiters in
/// a string cannot change the surrounding scope. Every mask preserves byte positions.
pub(crate) struct SourceLine<'a> {
    pub original: &'a str,
    pub code: String,
    pub syntax: String,
    pub comment: String,
}

fn mask(bytes: &mut [u8], from: usize, to: usize) {
    for byte in &mut bytes[from..to] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

fn raw_string_start(bytes: &[u8], pos: usize) -> Option<(usize, usize)> {
    let prefix = if bytes.get(pos..pos + 2) == Some(b"br") || bytes.get(pos..pos + 2) == Some(b"cr")
    {
        2
    } else if bytes.get(pos) == Some(&b'r') {
        1
    } else {
        return None;
    };
    if pos > 0 && (bytes[pos - 1].is_ascii_alphanumeric() || bytes[pos - 1] == b'_') {
        return None;
    }
    let mut cursor = pos + prefix;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'"')).then_some((cursor + 1, cursor - pos - prefix))
}

fn char_literal_end(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut cursor = pos + 1;
    if *bytes.get(cursor)? == b'\\' {
        cursor += 1;
        match *bytes.get(cursor)? {
            b'x' => cursor += 3,
            b'u' if bytes.get(cursor + 1) == Some(&b'{') => {
                cursor += 2;
                while bytes.get(cursor).is_some_and(|byte| *byte != b'}') {
                    cursor += 1;
                }
                cursor += 1;
            }
            _ => cursor += 1,
        }
    } else {
        let first = *bytes.get(cursor)?;
        if first == b'\n' || first == b'\r' || first == b'\'' {
            return None;
        }
        cursor += if first < 128 {
            1
        } else if first & 0xe0 == 0xc0 {
            2
        } else if first & 0xf0 == 0xe0 {
            3
        } else {
            4
        };
    }
    (bytes.get(cursor) == Some(&b'\'')).then_some(cursor + 1)
}

pub(crate) fn lines(source: &str) -> Vec<SourceLine<'_>> {
    let bytes = source.as_bytes();
    let mut code = bytes.to_vec();
    let mut syntax = bytes.to_vec();
    let mut comments: Vec<u8> = bytes
        .iter()
        .map(|byte| if *byte == b'\n' { b'\n' } else { b' ' })
        .collect();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let start = cursor;
        if bytes.get(cursor..cursor + 2) == Some(b"//") {
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            comments[start..cursor].copy_from_slice(&bytes[start..cursor]);
            mask(&mut code, start, cursor);
            mask(&mut syntax, start, cursor);
        } else if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            cursor += 2;
            let mut depth = 1;
            while cursor < bytes.len() && depth > 0 {
                if bytes.get(cursor..cursor + 2) == Some(b"/*") {
                    depth += 1;
                    cursor += 2;
                } else if bytes.get(cursor..cursor + 2) == Some(b"*/") {
                    depth -= 1;
                    cursor += 2;
                } else {
                    cursor += 1;
                }
            }
            mask(&mut code, start, cursor);
            mask(&mut syntax, start, cursor);
        } else if let Some((body, hashes)) = raw_string_start(bytes, cursor) {
            cursor = body;
            while cursor < bytes.len() {
                if bytes[cursor] == b'"'
                    && bytes
                        .get(cursor + 1..cursor + 1 + hashes)
                        .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
                {
                    cursor += 1 + hashes;
                    break;
                }
                cursor += 1;
            }
            mask(&mut syntax, start, cursor);
        } else if bytes[cursor] == b'"' {
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == b'"' {
                    cursor += 1;
                    break;
                } else {
                    cursor += 1;
                }
            }
            mask(&mut syntax, start, cursor);
        } else if bytes[cursor] == b'\'' {
            if let Some(end) = char_literal_end(bytes, cursor) {
                cursor = end;
                mask(&mut syntax, start, cursor);
            } else {
                cursor += 1;
            }
        } else {
            cursor += 1;
        }
    }

    // Only ASCII token bytes are replaced, and masked UTF-8 bytes are replaced together.
    let code = String::from_utf8(code).expect("mask preserves UTF-8");
    let syntax = String::from_utf8(syntax).expect("mask preserves UTF-8");
    let comments = String::from_utf8(comments).expect("comments preserve UTF-8");
    source
        .split('\n')
        .zip(code.split('\n'))
        .zip(syntax.split('\n'))
        .zip(comments.split('\n'))
        .map(|(((original, code), syntax), comment)| SourceLine {
            original,
            code: code.to_owned(),
            syntax: syntax.to_owned(),
            comment: comment.to_owned(),
        })
        .collect()
}
