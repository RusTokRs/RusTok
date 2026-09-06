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
        || !digits
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }

    Some(format!("#{digits}"))
}

/// Maps a validated hex color to a finite, build-time-visible utility-class palette.
///
/// This preserves a rough hue relationship without emitting a runtime CSS declaration. Invalid,
/// absent or nearly neutral values use the reviewed default gradient. Alpha components are ignored
/// because the host class owns opacity.
pub fn css_hex_accent_class(value: Option<&str>) -> &'static str {
    let Some((red, green, blue)) = value.and_then(css_hex_rgb) else {
        return "bg-gradient-to-b from-sky-500 to-amber-500";
    };
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    if maximum.saturating_sub(minimum) <= 24 {
        return "bg-slate-500";
    }

    if maximum == red {
        if green >= blue {
            let warm_threshold = ((u16::from(red) * 3) / 4) as u8;
            if green >= warm_threshold {
                "bg-amber-500"
            } else {
                "bg-rose-500"
            }
        } else {
            "bg-fuchsia-500"
        }
    } else if maximum == green {
        if blue > red {
            "bg-cyan-500"
        } else {
            "bg-emerald-500"
        }
    } else if red > green {
        "bg-violet-500"
    } else {
        "bg-sky-500"
    }
}

/// Converts the forum admin's legacy single-property accent representation into the finite class
/// palette without attaching the CSS declaration to the DOM.
///
/// Only the exact `background:<value>;` envelope is inspected. The nested value must still pass the
/// strict hex grammar; gradients, additional declarations and malformed envelopes use the reviewed
/// fallback class.
pub fn css_background_accent_class(value: &str) -> &'static str {
    let color = value
        .trim()
        .strip_prefix("background:")
        .and_then(|value| value.strip_suffix(';'))
        .map(str::trim);
    css_hex_accent_class(color)
}

fn css_hex_rgb(value: &str) -> Option<(u8, u8, u8)> {
    let normalized = normalize_css_hex_color(value)?;
    let digits = normalized.strip_prefix('#')?;
    match digits.len() {
        3 | 4 => {
            let mut values = digits.chars().take(3).map(|character| {
                character
                    .to_digit(16)
                    .map(|nibble| (nibble as u8).saturating_mul(17))
            });
            Some((values.next()??, values.next()??, values.next()??))
        }
        6 | 8 => Some((
            u8::from_str_radix(&digits[0..2], 16).ok()?,
            u8::from_str_radix(&digits[2..4], 16).ok()?,
            u8::from_str_radix(&digits[4..6], 16).ok()?,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{css_background_accent_class, css_hex_accent_class, normalize_css_hex_color};

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

    #[test]
    fn maps_hex_colors_to_a_finite_accent_palette() {
        for (raw, expected) in [
            (Some("#ff0000"), "bg-rose-500"),
            (Some("#ffff00"), "bg-amber-500"),
            (Some("#00ff00"), "bg-emerald-500"),
            (Some("#00ffff"), "bg-cyan-500"),
            (Some("#0000ff"), "bg-sky-500"),
            (Some("#8000ff"), "bg-violet-500"),
            (Some("#ff00ff"), "bg-fuchsia-500"),
            (Some("#777"), "bg-slate-500"),
        ] {
            assert_eq!(css_hex_accent_class(raw), expected, "mapped {raw:?}");
        }
        assert!(css_hex_accent_class(None).contains("from-sky-500"));
        assert!(css_hex_accent_class(Some("#fff;--owned:1")).contains("from-sky-500"));
    }

    #[test]
    fn maps_legacy_background_envelopes_without_rendering_css_text() {
        assert_eq!(
            css_background_accent_class("background:#0ea5e9;"),
            "bg-sky-500"
        );
        for raw in [
            "background:linear-gradient(180deg,#0ea5e9 0%,#f59e0b 100%);",
            "background:#fff;--owned:1;",
            "color:#fff;",
            "",
        ] {
            assert!(css_background_accent_class(raw).contains("from-sky-500"));
        }
    }
}
