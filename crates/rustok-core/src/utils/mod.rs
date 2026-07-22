//! General Utilities
//!
//! Common helper functions and utilities used throughout RusToK.
//!
//! # Features
//!
//! - **String utilities**: Common string manipulations
//! - **Collection utilities**: Helper methods for collections
//! - **Time utilities**: Common time operations
//! - **Validation helpers**: Common validation functions
//! - **Encoding utilities**: Base64, hex encoding/decoding
//!
//! # Example
//!
//! ```rust
//! use rustok_core::utils::{slugify, truncate, parse_duration};
//!
//! let slug = slugify("Hello World!");
//! assert_eq!(slug, "hello-world");
//!
//! let truncated = truncate("Long text here", 10);
//! assert_eq!(truncated, "Long te...");
//! ```

use std::collections::HashMap;
use std::time::Duration;

/// Convert a string to a URL-friendly slug
pub fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .replace([' ', '_'], "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Truncate a string to a maximum length, adding ellipsis if truncated
pub fn truncate(input: &str, max_len: usize) -> String {
    if input.len() <= max_len {
        input.to_string()
    } else {
        let truncated = &input[..max_len.saturating_sub(3)];
        format!("{}...", truncated)
    }
}

/// Parse a duration string (e.g., "1h30m", "2d", "30s")
pub fn parse_duration(input: &str) -> Option<Duration> {
    let chars = input.chars().peekable();
    let mut total_seconds: u64 = 0;
    let mut current_num: u64 = 0;

    for ch in chars {
        if ch.is_ascii_digit() {
            current_num = current_num * 10 + ch.to_digit(10)? as u64;
        } else {
            let multiplier = match ch {
                's' | 'S' => 1,
                'm' | 'M' => 60,
                'h' | 'H' => 60 * 60,
                'd' | 'D' => 24 * 60 * 60,
                'w' | 'W' => 7 * 24 * 60 * 60,
                _ => return None,
            };
            total_seconds += current_num * multiplier;
            current_num = 0;
        }
    }

    if current_num > 0 {
        // Number without suffix - treat as seconds
        total_seconds += current_num;
    }

    if total_seconds > 0 {
        Some(Duration::from_secs(total_seconds))
    } else {
        None
    }
}

/// Format a duration for display
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 60 * 60 {
        format!("{}m", secs / 60)
    } else if secs < 24 * 60 * 60 {
        format!("{}h", secs / (60 * 60))
    } else {
        format!("{}d", secs / (24 * 60 * 60))
    }
}

/// Check if a string is a valid email address (basic validation)
pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    if email.is_empty() || email.len() > 254 {
        return false;
    }

    // Basic email pattern
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() || local.len() > 64 {
        return false;
    }

    if domain.is_empty() || !domain.contains('.') {
        return false;
    }

    // Check for invalid characters
    let valid_local = local
        .chars()
        .all(|c| c.is_alphanumeric() || "._-+%".contains(c));
    let valid_domain = domain
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-');

    valid_local && valid_domain
}

/// Check if a string is a valid UUID
pub fn is_valid_uuid(input: &str) -> bool {
    uuid::Uuid::parse_str(input).is_ok()
}

/// Check if a string is a valid URL
pub fn is_valid_url(input: &str) -> bool {
    url::Url::parse(input).is_ok()
}

/// Sanitize a string for safe HTML display
pub fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Generate a random string of specified length
pub fn random_string(len: usize) -> String {
    use rand::RngExt;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let mut rng = rand::rng();
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Encode bytes to base64
pub fn base64_encode(input: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(input)
}

/// Decode base64 string
pub fn base64_decode(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.decode(input)
}

/// Encode bytes to hex
pub fn hex_encode(input: &[u8]) -> String {
    hex::encode(input)
}

/// Decode hex string
pub fn hex_decode(input: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(input)
}

/// Hash a string using a simple algorithm (for non-cryptographic purposes)
pub fn simple_hash(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

/// Get current timestamp in milliseconds
pub fn now_millis() -> u64 {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Get current timestamp in seconds
pub fn now_seconds() -> u64 {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Chunk a vector into smaller vectors of specified size
pub fn chunk<T: Clone>(input: Vec<T>, size: usize) -> Vec<Vec<T>> {
    input
        .chunks(size.max(1))
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// Remove duplicates from a vector while preserving order
pub fn dedup<T: Eq + Clone>(input: Vec<T>) -> Vec<T> {
    let mut seen = Vec::new();
    let mut result = Vec::new();

    for item in input {
        if !seen.contains(&item) {
            seen.push(item.clone());
            result.push(item);
        }
    }

    result
}

/// Find the first matching element by predicate
pub fn find_first<T>(input: &[T], predicate: impl Fn(&T) -> bool) -> Option<&T> {
    input.iter().find(|item| predicate(item))
}

/// Group elements by a key function
pub fn group_by<T, K, F>(input: Vec<T>, key_fn: F) -> HashMap<K, Vec<T>>
where
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
{
    let mut groups: HashMap<K, Vec<T>> = HashMap::new();

    for item in input {
        let key = key_fn(&item);
        groups.entry(key).or_default().push(item);
    }

    groups
}

/// Check if all elements match a predicate
pub fn all<T>(input: &[T], predicate: impl Fn(&T) -> bool) -> bool {
    input.iter().all(predicate)
}

/// Check if any element matches a predicate
pub fn any<T>(input: &[T], predicate: impl Fn(&T) -> bool) -> bool {
    input.iter().any(predicate)
}

/// Map and filter in one operation
pub fn filter_map<T, U>(input: Vec<T>, f: impl Fn(T) -> Option<U>) -> Vec<U> {
    input.into_iter().filter_map(f).collect()
}

/// Partition elements based on a predicate
pub fn partition<T>(input: Vec<T>, predicate: impl Fn(&T) -> bool) -> (Vec<T>, Vec<T>) {
    let mut left = Vec::new();
    let mut right = Vec::new();

    for item in input {
        if predicate(&item) {
            left.push(item);
        } else {
            right.push(item);
        }
    }

    (left, right)
}

/// Convert a `Vec<Result<T, E>>` into `Result<Vec<T>, E>`
pub fn collect_results<T, E>(input: Vec<Result<T, E>>) -> Result<Vec<T>, E> {
    let mut results = Vec::with_capacity(input.len());

    for result in input {
        match result {
            Ok(value) => results.push(value),
            Err(e) => return Err(e),
        }
    }

    Ok(results)
}

/// Convert snake_case to camelCase
pub fn to_camel_case(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for ch in input.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Convert camelCase to snake_case
pub fn to_snake_case(input: &str) -> String {
    let mut result = String::new();

    for (i, ch) in input.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }

    result
}

/// Capitalize the first letter of a string
pub fn capitalize(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str()
        }
    }
}

/// Pluralize a word (simple English rules)
pub fn pluralize(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_string()
    } else {
        // Simple rules - not comprehensive
        let word = word.to_lowercase();
        if word.ends_with("s")
            || word.ends_with("x")
            || word.ends_with("ch")
            || word.ends_with("sh")
        {
            format!("{}es", word)
        } else if word.ends_with("y") && !word.ends_with("ay") && !word.ends_with("ey") {
            format!("{}ies", &word[..word.len() - 1])
        } else {
            format!("{}s", word)
        }
    }
}

/// Parse a boolean from various string representations
pub fn parse_bool(input: &str) -> Option<bool> {
    match input.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" | "enabled" | "y" => Some(true),
        "false" | "no" | "0" | "off" | "disabled" | "n" => Some(false),
        _ => None,
    }
}

/// Safely get a value from a HashMap with a default
pub fn get_or_default<'a, K, V>(map: &'a HashMap<K, V>, key: &K, default: &'a V) -> &'a V
where
    K: Eq + std::hash::Hash,
{
    map.get(key).unwrap_or(default)
}

/// Merge two HashMaps, with the second taking precedence for conflicting keys
pub fn merge_maps<K, V>(mut left: HashMap<K, V>, right: HashMap<K, V>) -> HashMap<K, V>
where
    K: Eq + std::hash::Hash,
{
    left.extend(right);
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Test  File!"), "test-file");
        assert_eq!(slugify("multiple---dashes"), "multiple-dashes");
        assert_eq!(slugify("UPPER CASE"), "upper-case");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello", 10), "Hello");
        assert_eq!(truncate("Hello World", 8), "Hello...");
        assert_eq!(truncate("Short", 3), "...");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("2d"), Some(Duration::from_secs(172800)));
        assert_eq!(parse_duration("60"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(86400)), "1d");
    }

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name@domain.co.uk"));
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("test@"));
    }

    #[test]
    fn test_is_valid_uuid() {
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_valid_uuid("not-a-uuid"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(
            html_escape("<script>alert('test')</script>"),
            "&lt;script&gt;alert(&#x27;test&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_random_string() {
        let s1 = random_string(10);
        let s2 = random_string(10);
        assert_eq!(s1.len(), 10);
        assert_eq!(s2.len(), 10);
        assert_ne!(s1, s2); // Very unlikely to be equal
    }

    #[test]
    fn test_base64() {
        let input = b"hello world";
        let encoded = base64_encode(input);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_hex() {
        let input = b"hello world";
        let encoded = hex_encode(input);
        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_chunk() {
        let input = vec![1, 2, 3, 4, 5, 6, 7];
        let chunks = chunk(input, 3);
        assert_eq!(chunks, vec![vec![1, 2, 3], vec![4, 5, 6], vec![7]]);
    }

    #[test]
    fn test_dedup() {
        let input = vec![1, 2, 2, 3, 3, 3];
        assert_eq!(dedup(input), vec![1, 2, 3]);
    }

    #[test]
    fn test_group_by() {
        let input = vec![1, 2, 3, 4, 5, 6];
        let grouped = group_by(input, |n| n % 2 == 0);
        assert_eq!(grouped.get(&false), Some(&vec![1, 3, 5]));
        assert_eq!(grouped.get(&true), Some(&vec![2, 4, 6]));
    }

    #[test]
    fn test_case_conversions() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_snake_case("helloWorld"), "hello_world");
        assert_eq!(capitalize("hello"), "Hello");
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize("item", 1), "item");
        assert_eq!(pluralize("item", 0), "items");
        assert_eq!(pluralize("item", 5), "items");
        assert_eq!(pluralize("box", 2), "boxes");
        assert_eq!(pluralize("city", 2), "cities");
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("yes"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("no"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }
}
