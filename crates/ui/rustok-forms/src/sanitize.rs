//! Form data sanitization and value normalization helpers.
//!
//! Provides functions to clean user input before validation or persistence,
//! such as trimming, slug generation, email normalization, number clamping,
//! phone cleaning, and basic HTML tag stripping.

use std::borrow::Cow;

/// Sanitizer operations that can be composed or applied to input values.
pub mod sanitize {
    use super::*;

    /// Trim leading and trailing ASCII and Unicode whitespace.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::trim;
    ///
    /// assert_eq!(trim("  hello world \n "), "hello world");
    /// ```
    pub fn trim(input: &str) -> &str {
        input.trim()
    }

    /// Convert a string to an ASCII URL/slug-friendly representation.
    ///
    /// Converts to lowercase, replaces non-alphanumeric ASCII characters with hyphens,
    /// collapses consecutive hyphens, and removes leading/trailing hyphens.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::slugify;
    ///
    /// assert_eq!(slugify("Hello, World! 2026"), "hello-world-2026");
    /// assert_eq!(slugify("  --multi---dash-- "), "multi-dash");
    /// ```
    pub fn slugify(input: &str) -> String {
        let mut slug = String::with_capacity(input.len());
        let mut prev_dash = true; // avoid leading dash

        for ch in input.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
                prev_dash = false;
            } else if !prev_dash {
                slug.push('-');
                prev_dash = true;
            }
        }

        if slug.ends_with('-') {
            slug.pop();
        }

        slug
    }

    /// Normalize an email address by trimming whitespace and lowercasing the domain part.
    ///
    /// In accordance with standard web practice, the entire email is trimmed, and
    /// the full string is lowercased for safe case-insensitive storage.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::normalize_email;
    ///
    /// assert_eq!(normalize_email(" User.Name@Example.COM  "), "user.name@example.com");
    /// ```
    pub fn normalize_email(input: &str) -> String {
        input.trim().to_lowercase()
    }

    /// Normalize a phone number by keeping only leading `+` and digits.
    ///
    /// Removes spaces, dashes, parentheses, dots, and alphabetic characters.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::normalize_phone;
    ///
    /// assert_eq!(normalize_phone("+1 (555) 234-5678"), "+15552345678");
    /// assert_eq!(normalize_phone("8 (800) 555-35-35"), "88005553535");
    /// ```
    pub fn normalize_phone(input: &str) -> String {
        let trimmed = input.trim();
        let mut res = String::with_capacity(trimmed.len());
        let mut chars = trimmed.chars().peekable();

        if let Some('+') = chars.peek() {
            res.push('+');
            chars.next();
        }

        for ch in chars {
            if ch.is_ascii_digit() {
                res.push(ch);
            }
        }

        res
    }

    /// Collapse consecutive whitespace runs into a single ASCII space character.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::collapse_whitespace;
    ///
    /// assert_eq!(collapse_whitespace("hello   \t\n  world"), "hello world");
    /// ```
    pub fn collapse_whitespace(input: &str) -> String {
        let mut res = String::with_capacity(input.len());
        let mut in_whitespace = false;

        for ch in input.trim().chars() {
            if ch.is_whitespace() {
                if !in_whitespace {
                    res.push(' ');
                    in_whitespace = true;
                }
            } else {
                res.push(ch);
                in_whitespace = false;
            }
        }

        res
    }

    /// Clamp a numeric value within `[min, max]`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::clamp_number;
    ///
    /// assert_eq!(clamp_number(15.0, 0.0, 10.0), 10.0);
    /// assert_eq!(clamp_number(-5.0, 0.0, 10.0), 0.0);
    /// assert_eq!(clamp_number(5.0, 0.0, 10.0), 5.0);
    /// ```
    pub fn clamp_number<T: PartialOrd + Copy>(val: T, min: T, max: T) -> T {
        if val < min {
            min
        } else if val > max {
            max
        } else {
            val
        }
    }

    /// Basic HTML tag stripper for text-only sanitize pipelines.
    ///
    /// Strips `<...>` markup from input strings. Note: for complex, security-critical
    /// HTML purification against XSS, use a dedicated HTML sanitizer like `ammonia`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rustok_forms::sanitize::strip_tags;
    ///
    /// assert_eq!(strip_tags("<p>Hello <b>World</b>!</p>"), "Hello World!");
    /// ```
    pub fn strip_tags(input: &str) -> Cow<'_, str> {
        if !input.contains('<') {
            return Cow::Borrowed(input);
        }

        let mut out = String::with_capacity(input.len());
        let mut in_tag = false;

        for ch in input.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                other if !in_tag => out.push(other),
                _ => {}
            }
        }

        Cow::Owned(out)
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize::*;

    #[test]
    fn test_trim() {
        assert_eq!(trim("   abc  "), "abc");
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World! 123"), "hello-world-123");
        assert_eq!(slugify("---Foo---Bar---"), "foo-bar");
    }

    #[test]
    fn test_normalize_email() {
        assert_eq!(normalize_email("  Test@Example.Com "), "test@example.com");
    }

    #[test]
    fn test_normalize_phone() {
        assert_eq!(normalize_phone("+1 (800) 123-4567"), "+18001234567");
        assert_eq!(normalize_phone("8-800-555-35-35"), "88005553535");
    }

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(collapse_whitespace("a  b \t c\n d"), "a b c d");
    }

    #[test]
    fn test_clamp_number() {
        assert_eq!(clamp_number(15, 0, 10), 10);
        assert_eq!(clamp_number(-1, 0, 10), 0);
        assert_eq!(clamp_number(5, 0, 10), 5);
    }

    #[test]
    fn test_strip_tags() {
        assert_eq!(strip_tags("hello world"), "hello world");
        assert_eq!(strip_tags("<p>Hello <b>world</b>!</p>"), "Hello world!");
    }
}
