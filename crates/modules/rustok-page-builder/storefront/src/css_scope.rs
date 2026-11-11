//! Confinement of landing CSS to the storefront container.
//!
//! This is the Leptos twin of `scopeLandingCss` in
//! `apps/next-frontend/packages/rustok-pages/src/components/css-scope.ts`. Both
//! must return the same text for the same input; the shared vectors in
//! `contracts/landing-css-scope-parity.json` are asserted by this module's tests
//! and by the package test of the Next.js storefront.

/// Selector of the scope root: the element every storefront surface of this
/// crate renders around the landing markup (`<div data-fly-page-body="true">`).
///
/// The root is an element the components render themselves, so it exists
/// whatever `class` a host passes to them. A class the host may replace is not
/// a usable root: a stylesheet scoped to a selector that matches nothing
/// applies to nothing.
pub const LANDING_SCOPE_SELECTOR: &str = "[data-fly-page-body]";

/// Confines landing CSS to its container with CSS `@scope (<scope_selector>)`
/// and maps the document-level selectors (`body`, `html`, `:root`) to `:scope`,
/// so styles of a landing apply to the landing and not to the host shell,
/// header or footer.
///
/// `@scope` needs Chrome/Edge 118+, Safari 17.4+ or Firefox 128+. A browser
/// without it drops the whole block: the landing renders unstyled there, and
/// still cannot restyle the host.
///
/// The publish policy already limits what the stylesheet may contain. This
/// function does not rely on that for containment: it tokenizes the stylesheet
/// by the rules of the CSS Syntax specification (comments, strings, escapes,
/// names, numbers, `url()` tokens and `{}`/`()`/`[]` blocks), drops a `}` that
/// would close the wrapper early, and closes whatever the stylesheet left
/// open, so the returned text is always one complete `@scope` block. The
/// result is also safe as the text of a `<style>` element: it never contains
/// `</style`.
///
/// Not covered: at-rules that define document-wide names (`@keyframes`,
/// `@font-face`, `@property`, …). Where a browser accepts them inside
/// `@scope`, the names stay global.
pub fn scope_landing_css(raw_css: &str, scope_selector: &str) -> String {
    let css: Vec<char> = trim_css_whitespace(raw_css).chars().collect();
    if css.is_empty() {
        return String::new();
    }
    // Only this function's own output passes through. A stylesheet that merely
    // starts with `@scope` may scope to something else, or be followed by
    // rules outside the block.
    if is_single_scope_block(&css, scope_selector) {
        return keep_inside_style_element(&css.iter().collect::<String>());
    }

    let scan = scan_blocks(&css, true);
    let mut completed = scan.text;
    if !scan.open.is_empty() {
        // The newline before the closers matters: a stylesheet may end with a
        // backslash, and backslash-newline is not an escape, so the closers
        // stay structure.
        completed.push('\n');
        completed.extend(scan.open.iter().rev().map(|block| block.closer()));
    }
    keep_inside_style_element(&format!(
        "@scope ({scope_selector}) {{\n  :scope {{\n    isolation: isolate;\n  }}\n{completed}\n}}"
    ))
}

/// The stylesheet is emitted as the text of a `<style>` element, which the
/// HTML parser ends at the first `</style`. Escaping the slash keeps that
/// sequence out of the markup: inside a CSS string or `url()` token `\/` is
/// still `/`, and anywhere else `</style` meant nothing to CSS. Only `</style`
/// is touched, so the inserted backslash can never land in front of a `*` and
/// dissolve a comment delimiter.
fn keep_inside_style_element(css: &str) -> String {
    const TAG_NAME: &[u8] = b"style";
    let bytes = css.as_bytes();
    let mut result = String::with_capacity(css.len());
    let mut copied = 0;
    let mut index = 0;
    while index + 1 < bytes.len() {
        let closes_style = bytes[index] == b'<'
            && bytes[index + 1] == b'/'
            && bytes
                .get(index + 2..index + 2 + TAG_NAME.len())
                .is_some_and(|name| name.eq_ignore_ascii_case(TAG_NAME));
        if closes_style {
            // `<` is ASCII, so `index + 1` is a character boundary.
            result.push_str(&css[copied..=index]);
            result.push('\\');
            copied = index + 1;
            index += 2;
        } else {
            index += 1;
        }
    }
    result.push_str(&css[copied..]);
    result
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum BlockKind {
    Group,
    Declaration,
    Paren,
    Bracket,
}

impl BlockKind {
    fn closer(self) -> char {
        match self {
            Self::Group | Self::Declaration => '}',
            Self::Paren => ')',
            Self::Bracket => ']',
        }
    }
}

struct BlockScan {
    /// The stylesheet with root selectors remapped and stray `}` removed.
    text: String,
    /// Blocks still open at the end of input, outermost first.
    open: Vec<BlockKind>,
    /// Index just past the `}` that closed the first top-level `{}` block.
    first_block_end: Option<usize>,
    /// Whether a `}` was found outside every block.
    stray_close: bool,
}

/// Walks `css` token by token and tracks blocks the way the CSS syntax
/// specification consumes simple blocks. With `remap`, root selectors in
/// selector position become `:scope`.
///
/// Tokens are recognised exactly where the tokenizer starts them. That is what
/// decides whether `url(` opens a `url()` token, whose content up to `)` is
/// opaque, or a function, whose content is ordinary structure: `#url(`,
/// `@url(`, `myurl(` and `3url(` are the latter, and `u\72l(` is the former.
/// Guessing it from the surrounding text lets a stylesheet show this scanner
/// one block structure and the browser another.
fn scan_blocks(css: &[char], remap: bool) -> BlockScan {
    let mut open: Vec<BlockKind> = Vec::new();
    let mut text = String::with_capacity(css.len());
    let mut prelude = String::new();
    let mut first_block_end = None;
    let mut stray_close = false;

    let mut i = 0;
    while i < css.len() {
        let ch = css[i];
        let previous = i.checked_sub(1).and_then(|index| at(css, index));

        // Tokens whose content is never structure: copied verbatim, and
        // terminated if the stylesheet ends inside one so that it cannot
        // swallow what is appended after it.
        if let Some(opaque) = comment_or_string_token(css, i) {
            text.extend(&css[i..opaque.end]);
            text.push_str(opaque.terminator);
            i = opaque.end;
            continue;
        }

        // Hash, at-keyword and numeric tokens: a name that is part of one
        // never starts `url(`.
        let plain_end = hash_at_keyword_or_number_end(css, i);
        if plain_end > i {
            text.extend(&css[i..plain_end]);
            prelude.extend(&css[i..plain_end]);
            i = plain_end;
            continue;
        }

        if starts_ident(css, i) {
            let name = consume_name(css, i);
            let followed_by_paren = at(css, name.end) == Some('(');
            if followed_by_paren
                && name.value.eq_ignore_ascii_case("url")
                && let Some(url) = unquoted_url_token(css, name.end + 1)
            {
                text.extend(&css[i..url.end]);
                text.push_str(url.terminator);
                i = url.end;
                continue;
            }
            let raw = &css[i..name.end];
            let is_root_element = (raw == ['b', 'o', 'd', 'y'] || raw == ['h', 't', 'm', 'l'])
                && !followed_by_paren
                && previous != Some('.')
                && previous != Some(':');
            if remap && is_root_element && is_selector_position(&open, &prelude) {
                text.push_str(":scope");
                prelude.push_str(":scope");
            } else {
                text.extend(raw);
                prelude.extend(raw);
            }
            i = name.end;
            continue;
        }

        if remap && ch == ':' && previous != Some(':') && is_selector_position(&open, &prelude) {
            let name = consume_name(css, i + 1);
            if css[i + 1..name.end] == ['r', 'o', 'o', 't'] && at(css, name.end) != Some('(') {
                text.push_str(":scope");
                prelude.push_str(":scope");
                i = name.end;
                continue;
            }
        }

        match ch {
            '{' => {
                open.push(if is_css_group_rule(trim_css_whitespace(&prelude)) {
                    BlockKind::Group
                } else {
                    BlockKind::Declaration
                });
                prelude.clear();
                text.push(ch);
            }
            '(' | '[' => {
                open.push(if ch == '(' {
                    BlockKind::Paren
                } else {
                    BlockKind::Bracket
                });
                prelude.push(ch);
                text.push(ch);
            }
            '}' | ')' | ']' => match open.last() {
                Some(top) if top.closer() == ch => {
                    open.pop();
                    if ch == '}' {
                        prelude.clear();
                        if open.is_empty() && first_block_end.is_none() {
                            first_block_end = Some(i + 1);
                        }
                    } else {
                        prelude.push(ch);
                    }
                    text.push(ch);
                }
                None if ch == '}' => {
                    // Outside every block a `}` would end the `@scope` wrapper
                    // and release the rest of the stylesheet onto the host
                    // page.
                    stray_close = true;
                }
                _ => {
                    // A closer that does not match the innermost block is an
                    // ordinary token to the parser.
                    text.push(ch);
                    if ch != '}' {
                        prelude.push(ch);
                    }
                }
            },
            ';' if !is_inside_parens_or_brackets(&open) => {
                prelude.clear();
                text.push(ch);
            }
            _ => {
                prelude.push(ch);
                text.push(ch);
            }
        }
        i += 1;
    }

    BlockScan {
        text,
        open,
        first_block_end,
        stray_close,
    }
}

struct OpaqueToken {
    /// Index just past the token.
    end: usize,
    /// What closes the token when the input ends inside it, otherwise empty.
    terminator: &'static str,
}

/// The comment or string token starting at `start`, if any.
fn comment_or_string_token(css: &[char], start: usize) -> Option<OpaqueToken> {
    let ch = at(css, start)?;

    if ch == '/' && at(css, start + 1) == Some('*') {
        let mut i = start + 2;
        while i + 1 < css.len() {
            if css[i] == '*' && css[i + 1] == '/' {
                return Some(OpaqueToken {
                    end: i + 2,
                    terminator: "",
                });
            }
            i += 1;
        }
        return Some(OpaqueToken {
            end: css.len(),
            terminator: "*/",
        });
    }

    if ch != '"' && ch != '\'' {
        return None;
    }
    let mut i = start + 1;
    while i < css.len() {
        let c = css[i];
        if c == '\\' {
            if i + 1 >= css.len() {
                // The input ends on a backslash: it would escape a single
                // closing quote.
                return Some(OpaqueToken {
                    end: css.len(),
                    terminator: if ch == '"' { "\"\"" } else { "''" },
                });
            }
            // An escaped newline continues the string, and CR LF is one
            // newline.
            i += if css[i + 1] == '\r' && at(css, i + 2) == Some('\n') {
                3
            } else {
                2
            };
        } else if c == ch {
            return Some(OpaqueToken {
                end: i + 1,
                terminator: "",
            });
        } else if is_newline(c) {
            // An unescaped newline ends the (bad) string; the newline is not
            // part of it.
            return Some(OpaqueToken {
                end: i,
                terminator: "",
            });
        } else {
            i += 1;
        }
    }
    Some(OpaqueToken {
        end: css.len(),
        terminator: if ch == '"' { "\"" } else { "'" },
    })
}

/// The rest of a `url()` token whose `(` ends just before `start`, or `None`
/// when a quoted string follows: then `url(` is a function token and its
/// argument an ordinary string token.
fn unquoted_url_token(css: &[char], start: usize) -> Option<OpaqueToken> {
    let mut i = start;
    while at(css, i).is_some_and(is_css_whitespace) {
        i += 1;
    }
    if matches!(at(css, i), Some('"' | '\'')) {
        return None;
    }

    let mut i = start;
    while i < css.len() {
        if css[i] == '\\' {
            if i + 1 >= css.len() {
                // The input ends on a backslash: it would escape a single
                // closing parenthesis.
                return Some(OpaqueToken {
                    end: css.len(),
                    terminator: "))",
                });
            }
            i += 2;
        } else if css[i] == ')' {
            return Some(OpaqueToken {
                end: i + 1,
                terminator: "",
            });
        } else {
            i += 1;
        }
    }
    Some(OpaqueToken {
        end: css.len(),
        terminator: ")",
    })
}

/// End of the hash (`#name`), at-keyword (`@name`) or numeric token (number,
/// percentage or dimension) starting at `start`; `start` itself when none
/// starts there. An exponent is left to be read as part of the unit, which
/// never changes where a later `url(` token can start.
fn hash_at_keyword_or_number_end(css: &[char], start: usize) -> usize {
    match at(css, start) {
        Some('#') => {
            if at(css, start + 1).is_some_and(is_name_char) || is_valid_escape(css, start + 1) {
                consume_name(css, start + 1).end
            } else {
                start
            }
        }
        Some('@') => {
            if starts_ident(css, start + 1) {
                consume_name(css, start + 1).end
            } else {
                start
            }
        }
        _ if starts_number(css, start) => {
            let mut i = start;
            if matches!(at(css, i), Some('+' | '-')) {
                i += 1;
            }
            while at(css, i).is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
            }
            if at(css, i) == Some('.') && at(css, i + 1).is_some_and(|c| c.is_ascii_digit()) {
                i += 2;
                while at(css, i).is_some_and(|c| c.is_ascii_digit()) {
                    i += 1;
                }
            }
            if starts_ident(css, i) {
                consume_name(css, i).end
            } else {
                i
            }
        }
        _ => start,
    }
}

fn starts_number(css: &[char], start: usize) -> bool {
    let is_digit_at = |index: usize| at(css, index).is_some_and(|c| c.is_ascii_digit());
    match at(css, start) {
        Some(c) if c.is_ascii_digit() => true,
        Some('.') => is_digit_at(start + 1),
        Some('+' | '-') => {
            is_digit_at(start + 1) || (at(css, start + 1) == Some('.') && is_digit_at(start + 2))
        }
        _ => false,
    }
}

/// Whether an ident sequence (a name, possibly written with escapes) starts
/// at `start`.
fn starts_ident(css: &[char], start: usize) -> bool {
    match at(css, start) {
        Some('-') => {
            let next = at(css, start + 1);
            next.is_some_and(is_name_start) || next == Some('-') || is_valid_escape(css, start + 1)
        }
        Some(c) => is_name_start(c) || is_valid_escape(css, start),
        None => false,
    }
}

struct ConsumedName {
    /// Index just past the name.
    end: usize,
    /// The name with its escapes decoded, as the tokenizer compares it.
    value: String,
}

fn consume_name(css: &[char], start: usize) -> ConsumedName {
    let mut i = start;
    let mut value = String::new();
    while let Some(c) = at(css, i) {
        if is_name_char(c) {
            value.push(c);
            i += 1;
        } else if is_valid_escape(css, i) {
            let (end, decoded) = consume_escape(css, i);
            value.push(decoded);
            i = end;
        } else {
            break;
        }
    }
    ConsumedName { end: i, value }
}

/// Consumes the escape whose backslash is at `start`; returns the index just
/// past it and the code point it stands for.
fn consume_escape(css: &[char], start: usize) -> (usize, char) {
    const REPLACEMENT: char = '\u{fffd}';
    let mut i = start + 1;
    let Some(first) = at(css, i) else {
        return (css.len(), REPLACEMENT);
    };
    if !first.is_ascii_hexdigit() {
        return (i + 1, first);
    }

    let mut code: u32 = 0;
    let mut digits = 0;
    while digits < 6 {
        let Some(digit) = at(css, i).and_then(|c| c.to_digit(16)) else {
            break;
        };
        code = code * 16 + digit;
        digits += 1;
        i += 1;
    }
    // One whitespace after the digits belongs to the escape; CR LF is one
    // whitespace.
    if at(css, i) == Some('\r') && at(css, i + 1) == Some('\n') {
        i += 2;
    } else if at(css, i).is_some_and(is_css_whitespace) {
        i += 1;
    }

    // `char::from_u32` rejects surrogates and values above U+10FFFF; CSS also
    // replaces NUL.
    let decoded = char::from_u32(code)
        .filter(|c| *c != '\0')
        .unwrap_or(REPLACEMENT);
    (i, decoded)
}

/// A backslash starts an escape unless a newline follows it (at the end of
/// input it does).
fn is_valid_escape(css: &[char], start: usize) -> bool {
    at(css, start) == Some('\\') && !at(css, start + 1).is_some_and(is_newline)
}

fn is_newline(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{c}')
}

/// CSS whitespace only: Unicode whitespace also covers code points that CSS
/// reads as part of a name.
fn is_css_whitespace(ch: char) -> bool {
    ch == ' ' || ch == '\t' || is_newline(ch)
}

fn trim_css_whitespace(value: &str) -> &str {
    value.trim_matches(is_css_whitespace)
}

/// Letters, `_`, every non-ASCII code point, and NUL (which CSS replaces with
/// U+FFFD).
fn is_name_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '\0' || ch >= '\u{80}'
}

fn is_name_char(ch: char) -> bool {
    is_name_start(ch) || ch.is_ascii_digit() || ch == '-'
}

fn at(css: &[char], index: usize) -> Option<char> {
    css.get(index).copied()
}

/// Whether the scan is inside a selector: not in a declaration block, not in
/// an attribute selector, and not in the prelude of an at-rule
/// (`@keyframes body`, `@media …`).
fn is_selector_position(open: &[BlockKind], prelude: &str) -> bool {
    open.iter()
        .all(|kind| matches!(kind, BlockKind::Group | BlockKind::Paren))
        && !prelude
            .trim_start_matches(is_css_whitespace)
            .starts_with('@')
}

fn is_inside_parens_or_brackets(open: &[BlockKind]) -> bool {
    matches!(open.last(), Some(BlockKind::Paren | BlockKind::Bracket))
}

/// Whether `css` is exactly one complete `@scope (<scope_selector>) { … }`
/// block.
fn is_single_scope_block(css: &[char], scope_selector: &str) -> bool {
    let Some(brace_index) = css.iter().position(|ch| *ch == '{') else {
        return false;
    };
    let prelude: String = css[..brace_index].iter().collect();
    let expected = format!("@scope ({})", collapse_css_whitespace(scope_selector));
    if collapse_css_whitespace(&prelude) != expected {
        return false;
    }
    let scan = scan_blocks(css, false);
    scan.open.is_empty() && !scan.stray_close && scan.first_block_end == Some(css.len())
}

/// Replaces every run of CSS whitespace with one space and trims the ends.
fn collapse_css_whitespace(value: &str) -> String {
    let mut collapsed = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if is_css_whitespace(ch) {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(ch);
        }
    }
    collapsed
}

fn is_css_group_rule(prelude: &str) -> bool {
    prelude.starts_with("@media")
        || prelude.starts_with("@supports")
        || prelude.starts_with("@container")
        || prelude.starts_with("@document")
        || (prelude.starts_with("@layer") && !prelude.contains(';'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const SELECTOR: &str = LANDING_SCOPE_SELECTOR;

    #[derive(Deserialize)]
    struct ParityVectors {
        cases: Vec<ParityCase>,
    }

    #[derive(Deserialize)]
    struct ParityCase {
        name: String,
        css: String,
        inner: String,
    }

    fn wrap(selector: &str, inner: &str) -> String {
        format!("@scope ({selector}) {{\n  :scope {{\n    isolation: isolate;\n  }}\n{inner}\n}}")
    }

    /// Asserts that `css` is exactly one top-level `{}` block. Written
    /// separately from the scanner above, over ASCII test inputs only.
    fn assert_single_scope_block(css: &str) {
        assert!(css.starts_with("@scope ("), "not an @scope block: {css}");
        let bytes = css.as_bytes();
        let mut stack: Vec<u8> = Vec::new();
        let mut closed_at = None;
        let mut i = 0;
        while i < bytes.len() {
            let ch = bytes[i];
            if bytes[i..].starts_with(b"/*") {
                let end = css[i + 2..]
                    .find("*/")
                    .unwrap_or_else(|| panic!("unterminated comment: {css}"));
                i += end + 4;
            } else if ch == b'"' || ch == b'\'' {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != ch && bytes[j] != b'\n' {
                    j += if bytes[j] == b'\\' { 2 } else { 1 };
                }
                i = if bytes.get(j) == Some(&ch) { j + 1 } else { j };
            } else if ch == b'\\' {
                i += 2;
            } else if bytes[i..].starts_with(b"url(")
                && !matches!(bytes.get(i + 4), Some(b'"' | b'\'' | b' '))
                && !(i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-'))
            {
                let mut j = i + 4;
                while j < bytes.len() && bytes[j] != b')' {
                    j += if bytes[j] == b'\\' { 2 } else { 1 };
                }
                assert!(j < bytes.len(), "unterminated url(): {css}");
                i = j + 1;
            } else {
                match ch {
                    b'{' => stack.push(b'}'),
                    b'(' => stack.push(b')'),
                    b'[' => stack.push(b']'),
                    b'}' | b')' | b']' => {
                        if stack.last() == Some(&ch) {
                            stack.pop();
                            if stack.is_empty() && ch == b'}' && closed_at.is_none() {
                                closed_at = Some(i);
                            }
                        } else {
                            assert!(!stack.is_empty(), "stray closer at top level: {css}");
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        assert!(stack.is_empty(), "unclosed blocks: {css}");
        assert_eq!(
            closed_at,
            Some(bytes.len() - 1),
            "content after the @scope block: {css}"
        );
    }

    #[test]
    fn matches_the_vectors_shared_with_the_next_storefront() {
        let vectors: ParityVectors = serde_json::from_str(include_str!(
            "../../contracts/landing-css-scope-parity.json"
        ))
        .expect("parity vectors");
        assert!(!vectors.cases.is_empty());
        for case in vectors.cases {
            assert_eq!(
                scope_landing_css(&case.css, SELECTOR),
                wrap(SELECTOR, &case.inner),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn wraps_and_remaps_root_selectors() {
        let raw = "body { margin: 0; background: #fff; }\nhtml, body { height: 100%; }\n.hero { color: red; }\ntbody { display: table-row-group; }\n.some-body { color: blue; }";
        let scoped = scope_landing_css(raw, SELECTOR);

        assert!(scoped.starts_with("@scope ([data-fly-page-body]) {"));
        assert!(scoped.contains(":scope {\n    isolation: isolate;"));
        assert!(scoped.contains(":scope { margin: 0; background: #fff; }"));
        assert!(scoped.contains(":scope, :scope { height: 100%; }"));
        assert!(scoped.contains(".hero { color: red; }"));
        // `tbody` and `.some-body` only contain the word.
        assert!(scoped.contains("tbody { display: table-row-group; }"));
        assert!(scoped.contains(".some-body { color: blue; }"));
    }

    #[test]
    fn preserves_comments_with_braces_and_id_selectors() {
        let raw = "/* { comment with braces } */\n#body { font-size: 14px; }\n/* body in comment */\nbody { padding: 8px; }";
        let scoped = scope_landing_css(raw, SELECTOR);

        assert!(scoped.contains("/* { comment with braces } */"));
        assert!(scoped.contains("#body { font-size: 14px; }"));
        assert!(scoped.contains("/* body in comment */"));
        assert!(scoped.contains(":scope { padding: 8px; }"));
    }

    #[test]
    fn empty_input_yields_no_stylesheet() {
        assert_eq!(scope_landing_css("", SELECTOR), "");
        assert_eq!(scope_landing_css(" \t\r\n\u{c}", SELECTOR), "");
    }

    #[test]
    fn passes_through_only_its_own_output() {
        let own = "@scope (.test) { :scope { color: green; } }";
        assert_eq!(scope_landing_css(own, ".test"), own);
        let once = scope_landing_css("body { margin: 0; } .hero { color: red; }", SELECTOR);
        assert_eq!(scope_landing_css(&once, SELECTOR), once);

        // A block scoped to something else is confined like any other rule.
        let foreign = scope_landing_css(own, ".other");
        assert!(foreign.starts_with("@scope (.other) {"));
        assert!(foreign.contains(own));

        // Rules after a leading `@scope` block must not reach the host page.
        let trailing = "@scope (.test) { a { color: red; } } header { display: none; }";
        let wrapped = scope_landing_css(trailing, ".test");
        assert_ne!(wrapped, trailing);
        assert_single_scope_block(&wrapped);
        assert!(wrapped.contains("header { display: none; }"));
    }

    #[test]
    fn output_is_one_complete_scope_block_for_unbalanced_input() {
        let cases = [
            // A stray `}` would close the wrapper and release the rest.
            "} header { display: none; }",
            ".a { color: red; } } header { display: none; }",
            // Unclosed blocks, strings, comments and url() tokens.
            ".a { color: red;",
            "@media (min-width: 1px) { .a { color: red;",
            ".a { content: \"unterminated",
            ".a { color: red; } /* unterminated",
            ".a { background: url(unterminated",
            ".a { width: calc((1px + 2px) ",
            ".a { color: red; } \\",
            // Braces that are not structure.
            ".a::before { content: \"}\"; } header { display: none; }",
            ".a::before { content: '\\'}'; } .b { color: blue; }",
            ".a { background: url(x}y.png); } .b { color: blue; }",
            ".a { background: url(x{y.png); } } header { display: none; }",
            ".a\\} { color: red; } .b { color: blue; }",
            "/* } */ .a { color: red; }",
            // A newline ends an unterminated string; the `}` after it is
            // structure again.
            ".a { content: \"open\n} } header { display: none; }",
            // A `}` inside an open parenthesis is an ordinary token.
            ".a { width: calc(1px } .b { color: blue; }",
        ];
        for css in cases {
            assert_single_scope_block(&scope_landing_css(css, SELECTOR));
        }
    }

    #[test]
    fn output_cannot_end_the_style_element_it_is_rendered_into() {
        let scoped = scope_landing_css(
            ".a::after { content: \"</style><script>alert(1)</script>\"; } /* </STYLE> */ .b { color: red; }",
            SELECTOR,
        );
        assert!(!scoped.to_ascii_lowercase().contains("</style"));
        assert!(scoped.contains("content: \"<\\/style><script>alert(1)</script>\";"));
        assert!(scoped.contains("/* <\\/STYLE> */ .b { color: red; }"));
        // Its own output still passes through unchanged.
        assert_eq!(scope_landing_css(&scoped, SELECTOR), scoped);
    }

    #[test]
    fn remaps_root_selectors_only_in_selector_position() {
        let raw = [
            "@keyframes body { from { opacity: 0; } to { opacity: 1; } }",
            ".a { animation: body 1s; }",
            "a[rel=html] { color: red; }",
            "a[data-kind=\"body\"] { color: red; }",
            ".b { background: url(body.png); }",
            "div:not(body) > html { color: blue; }",
            "@media (min-width: 1px) { .c { color: red; .d { color: blue; } } body { margin: 0; } }",
        ]
        .join("\n");
        let scoped = scope_landing_css(&raw, SELECTOR);

        assert!(scoped.contains("@keyframes body { from { opacity: 0; } to { opacity: 1; } }"));
        assert!(scoped.contains(".a { animation: body 1s; }"));
        assert!(scoped.contains("a[rel=html] { color: red; }"));
        assert!(scoped.contains("a[data-kind=\"body\"] { color: red; }"));
        assert!(scoped.contains(".b { background: url(body.png); }"));
        assert!(scoped.contains("div:not(:scope) > :scope { color: blue; }"));
        assert!(scoped.contains(".d { color: blue; } } :scope { margin: 0; } }"));
    }

    #[test]
    fn handles_cyrillic_nested_group_rules_and_root() {
        let raw = r#"
/* Привет, мир! Комментарий на кириллице */
:root {
  --primary-color: #007bff;
}
body {
  font-family: 'Roboto', sans-serif;
  --custom-ident: body;
}
@media (max-width: 768px) {
  body {
    padding: 10px;
  }
}
@supports (display: flex) {
  @media (min-width: 1200px) {
    html, body {
      display: flex;
    }
  }
}
"#;
        let scoped = scope_landing_css(raw, SELECTOR);

        assert!(scoped.contains("/* Привет, мир! Комментарий на кириллице */"));
        assert!(scoped.contains(":scope {\n  --primary-color: #007bff;\n}"));
        assert!(scoped.contains(
            ":scope {\n  font-family: 'Roboto', sans-serif;\n  --custom-ident: body;\n}"
        ));
        assert!(
            scoped.contains("@media (max-width: 768px) {\n  :scope {\n    padding: 10px;\n  }\n}")
        );
        assert!(scoped.contains(
            "@media (min-width: 1200px) {\n    :scope, :scope {\n      display: flex;\n    }\n  }"
        ));
    }
}
