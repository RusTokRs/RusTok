//! Value sanitization and normalization utilities for form inputs.

/// Normalizes and cleans form input values prior to validation or submission.
pub mod sanitize {
    /// Trim leading and trailing whitespace.
    pub fn trim(s: &str) -> String {
        s.trim().to_string()
    }

    /// Convert string to lowercase.
    pub fn lowercase(s: &str) -> String {
        s.to_lowercase()
    }

    /// Convert string to uppercase.
    pub fn uppercase(s: &str) -> String {
        s.to_uppercase()
    }

    /// Normalize an email address by trimming whitespace and lowercasing.
    pub fn normalize_email(s: &str) -> String {
        s.trim().to_lowercase()
    }

    /// Convert a human-readable title or string into a kebab-case URL slug.
    ///
    /// Replaces non-alphanumeric characters with hyphens, collapses consecutive hyphens,
    /// and trims boundary hyphens.
    pub fn slugify(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut last_was_dash = true;

        for ch in s.chars() {
            if ch.is_ascii_alphanumeric() {
                result.push(ch.to_ascii_lowercase());
                last_was_dash = false;
            } else if !last_was_dash {
                result.push('-');
                last_was_dash = true;
            }
        }

        while result.ends_with('-') {
            result.pop();
        }

        result
    }

    /// Retain only ASCII digits in a string.
    pub fn digits_only(s: &str) -> String {
        s.chars().filter(|c| c.is_ascii_digit()).collect()
    }

    /// Retain only alphanumeric characters in a string.
    pub fn alphanumeric_only(s: &str) -> String {
        s.chars().filter(|c| c.is_alphanumeric()).collect()
    }

    /// Normalize a phone number by retaining leading `+` and all ASCII digits.
    pub fn normalize_phone(s: &str) -> String {
        let trimmed = s.trim();
        let has_plus = trimmed.starts_with('+');
        let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
        if has_plus {
            format!("+{digits}")
        } else {
            digits
        }
    }

    /// Remove basic HTML/XML tags from a string.
    pub fn strip_tags(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut in_tag = false;

        for ch in s.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }

        result
    }

    /// Clamp a 64-bit floating point number between `min` and `max` inclusive.
    pub fn clamp_f64(val: f64, min: f64, max: f64) -> f64 {
        if val < min {
            min
        } else if val > max {
            max
        } else {
            val
        }
    }

    /// Clamp a 64-bit signed integer between `min` and `max` inclusive.
    pub fn clamp_i64(val: i64, min: i64, max: i64) -> i64 {
        if val < min {
            min
        } else if val > max {
            max
        } else {
            val
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("  My First Post - 2026!  "), "my-first-post-2026");
        assert_eq!(slugify("---special--chars---"), "special-chars");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn test_normalize_email() {
        assert_eq!(normalize_email("  User@Example.COM  "), "user@example.com");
    }

    #[test]
    fn test_normalize_phone() {
        assert_eq!(normalize_phone("+1 (555) 000-1234"), "+15550001234");
        assert_eq!(normalize_phone("8 (800) 555-35-35"), "88005553535");
    }

    #[test]
    fn test_digits_only() {
        assert_eq!(digits_only("42-ABC-99!"), "4299");
    }

    #[test]
    fn test_strip_tags() {
        assert_eq!(strip_tags("<p>Hello <b>World</b></p>"), "Hello World");
        assert_eq!(strip_tags("Safe text"), "Safe text");
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp_f64(5.0, 10.0, 20.0), 10.0);
        assert_eq!(clamp_f64(25.0, 10.0, 20.0), 20.0);
        assert_eq!(clamp_f64(15.0, 10.0, 20.0), 15.0);

        assert_eq!(clamp_i64(0, 1, 10), 1);
        assert_eq!(clamp_i64(15, 1, 10), 10);
    }
}
