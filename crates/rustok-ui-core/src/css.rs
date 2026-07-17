/// Normalizes a CSS color for contexts that must render a single trusted color token.
///
/// The contract is intentionally narrower than the full CSS color grammar. Allowing functions,
/// named colors or arbitrary declarations would make it possible for persisted UI configuration
/// to escape a single property through characters such as `;`. Supported forms are `#RGB`,
/// `#RGBA`, `#RRGGBB` and `#RRGGBBAA`.
pub fn normalize_css_hex_color(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let digits = trimmed.strip_prefix('#')?;
    if !matches!(digits.len(), 3 | 4 | 6 | 8)
        || !digits.chars().all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }

    Some(format!("#{digits}"))
}

#[cfg(test)]
mod tests {
    use super::normalize_css_hex_color;

    #[test]
    fn accepts_only_bounded_hex_color_tokens() {
        for (raw, expected) in [
            (" #fff ", "#fff"),
            ("#0EA5E9", "#0EA5E9"),
            ("#abcd", "#abcd"),
            ("#11223344", "#11223344"),
        ] {
            assert_eq!(normalize_css_hex_color(raw).as_deref(), Some(expected));
        }
    }

    #[test]
    fn rejects_css_declarations_functions_and_invalid_lengths() {
        for raw in [
            "red",
            "rgb(1 2 3)",
            "#12",
            "#12345",
            "#ggg",
            "#fff;background:url(https://attacker.invalid/x)",
            "#fff;--token:owned",
            "",
        ] {
            assert_eq!(normalize_css_hex_color(raw), None, "accepted {raw:?}");
        }
    }
}
