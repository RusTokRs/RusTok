//! Project design tokens (CSS custom properties), owned by Fly.
//!
//! A project may carry a `flyTheme` extension: `{ "tokens": { "brand": "#0b5fff", ... } }`.
//! Fly decodes and validates it here — token names become `--{name}` custom properties and
//! token values must stay inside the safe CSS value subset — and the renderer emits one
//! `:root { ... }` rule ahead of the project styles. Style declarations reference tokens
//! with the standard `var(--brand)` syntax, which the declaration sanitizer already admits.
//!
//! Before this module, projects had per-component declarations and class rules only: no
//! variables, no site-wide theme (audit P0-3). Tokens are project data, not style rules,
//! because they must survive GrapesJS round-trips untouched by rule identity; the codec's
//! flattened extension map already guarantees that.

use crate::ProjectDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FLY_THEME_FIELD: &str = "flyTheme";
pub const MAXIMUM_THEME_TOKENS: usize = 256;
pub const MAXIMUM_TOKEN_NAME_LENGTH: usize = 64;
pub const MAXIMUM_TOKEN_VALUE_LENGTH: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignToken {
    pub name: String,
    pub value: String,
}

impl DesignToken {
    /// The CSS custom property this token defines, e.g. `--brand`.
    pub fn css_variable(&self) -> String {
        format!("--{}", self.name)
    }
}

/// Decode and validate the project's design tokens, sorted by name. A project without a
/// `flyTheme` extension has no tokens; a malformed one is an error, never a partial theme.
pub fn decode_theme_tokens(document: &ProjectDocument) -> Result<Vec<DesignToken>, String> {
    let Some(theme) = document.project.extensions.get(FLY_THEME_FIELD) else {
        return Ok(Vec::new());
    };
    let tokens = theme
        .get("tokens")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "flyTheme.tokens must be an object of token name to value".to_string())?;
    if tokens.len() > MAXIMUM_THEME_TOKENS {
        return Err(format!(
            "flyTheme declares {} tokens, exceeding the maximum of {MAXIMUM_THEME_TOKENS}",
            tokens.len()
        ));
    }
    let mut decoded = Vec::with_capacity(tokens.len());
    for (name, value) in tokens {
        validate_token_name(name)?;
        let value = value
            .as_str()
            .ok_or_else(|| format!("design token `{name}` must have a string value"))?;
        validate_token_value(name, value)?;
        decoded.push(DesignToken {
            name: name.clone(),
            value: value.trim().to_string(),
        });
    }
    decoded.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(decoded)
}

/// Token names become CSS custom property names verbatim after the `--` prefix, so the
/// charset is deliberately identifier-like: lowercase ASCII, digits, and hyphens.
pub fn validate_token_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAXIMUM_TOKEN_NAME_LENGTH {
        return Err(format!(
            "design token name `{name}` must be 1..={MAXIMUM_TOKEN_NAME_LENGTH} characters"
        ));
    }
    let mut characters = name.chars();
    let first = characters.next().unwrap_or_default();
    if !first.is_ascii_lowercase() {
        return Err(format!(
            "design token name `{name}` must start with a lowercase letter"
        ));
    }
    if !characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) {
        return Err(format!(
            "design token name `{name}` may contain only lowercase letters, digits, and hyphens"
        ));
    }
    if name.ends_with('-') || name.contains("--") {
        return Err(format!(
            "design token name `{name}` must not end with or repeat hyphens"
        ));
    }
    Ok(())
}

/// Token values are emitted verbatim into a `:root` rule, so they must stay inside the
/// same safe CSS value subset as style declarations: no structural breakout, no code
/// execution vectors. `var(--other)` references between tokens are allowed.
pub fn validate_token_value(name: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAXIMUM_TOKEN_VALUE_LENGTH {
        return Err(format!(
            "design token `{name}` value must be 1..={MAXIMUM_TOKEN_VALUE_LENGTH} characters"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!(
            "design token `{name}` value contains control characters"
        ));
    }
    if value.contains('\\')
        || value.contains('<')
        || value.contains('>')
        || value.contains(';')
        || value.contains('{')
        || value.contains('}')
        || value.contains("/*")
        || value.contains("*/")
    {
        return Err(format!(
            "design token `{name}` value contains characters that could break out of a CSS rule"
        ));
    }
    let compact = value
        .to_ascii_lowercase()
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact.contains("expression(")
        || compact.contains("javascript:")
        || compact.contains("@import")
        || compact.contains("behavior:")
        || compact.contains("-moz-binding")
        || compact.contains("url(")
    {
        return Err(format!(
            "design token `{name}` value uses a forbidden CSS construct"
        ));
    }
    Ok(())
}

/// The `:root` rule for a token set, or `None` when empty. Inputs must already be
/// validated (see [`decode_theme_tokens`]); this only formats them.
pub fn theme_root_rule(tokens: &[DesignToken]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }
    let mut rule = String::from(":root{");
    for token in tokens {
        rule.push_str(&token.css_variable());
        rule.push(':');
        rule.push_str(token.value.trim());
        rule.push(';');
    }
    rule.push('}');
    Some(rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document_with_theme(theme: serde_json::Value) -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "flyTheme": theme,
            "pages": [{ "id": "home", "component": { "id": "root", "type": "wrapper" } }]
        }))
        .expect("document")
    }

    #[test]
    fn decodes_tokens_sorted_and_builds_root_rule() {
        let document = document_with_theme(json!({
            "tokens": { "space-md": "1rem", "brand": "#0b5fff" }
        }));
        let tokens = decode_theme_tokens(&document).expect("tokens");
        assert_eq!(tokens[0].name, "brand");
        assert_eq!(tokens[0].css_variable(), "--brand");
        assert_eq!(
            theme_root_rule(&tokens).as_deref(),
            Some(":root{--brand:#0b5fff;--space-md:1rem;}")
        );
    }

    #[test]
    fn missing_theme_is_empty_not_an_error() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "id": "home", "component": { "id": "root", "type": "wrapper" } }]
        }))
        .expect("document");
        assert!(decode_theme_tokens(&document).expect("tokens").is_empty());
        assert_eq!(theme_root_rule(&[]), None);
    }

    #[test]
    fn rejects_breakout_values_and_bad_names() {
        for (tokens, case) in [
            (json!({ "Brand": "#fff" }), "uppercase name"),
            (
                json!({ "brand": "red; } body { display:none" }),
                "rule breakout",
            ),
            (
                json!({ "brand": "url(https://evil.example/x)" }),
                "url value",
            ),
            (
                json!({ "brand": "expression(alert(1))" }),
                "expression value",
            ),
        ] {
            let document = document_with_theme(json!({ "tokens": tokens }));
            assert!(
                decode_theme_tokens(&document).is_err(),
                "case must fail: {case}"
            );
        }
    }

    #[test]
    fn allows_token_references_between_tokens() {
        let document = document_with_theme(json!({
            "tokens": { "brand": "#0b5fff", "accent": "var(--brand)" }
        }));
        assert!(decode_theme_tokens(&document).is_ok());
    }
}

/// Mutations of the project design-token set, executed as editor commands so token
/// authoring goes through the same transaction/validation/history path as every other
/// edit (instead of hosts poking the `flyTheme` extension directly).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThemeCommand {
    /// Replace the whole token set (an empty set removes the theme).
    Replace { tokens: Vec<DesignToken> },
    /// Insert or update one token by name.
    Upsert { token: DesignToken },
    /// Remove one token by name; removing the last one removes the theme.
    Remove { name: String },
}

pub fn apply_theme_command(
    document: &mut ProjectDocument,
    command: &ThemeCommand,
) -> crate::FlyResult<()> {
    let mut tokens = decode_theme_tokens(document)
        .map_err(|message| crate::FlyError::Decode(format!("invalid flyTheme: {message}")))?;
    match command {
        ThemeCommand::Replace {
            tokens: replacement,
        } => {
            tokens = replacement.clone();
        }
        ThemeCommand::Upsert { token } => {
            validate_token_name(&token.name)
                .and_then(|_| validate_token_value(&token.name, &token.value))
                .map_err(crate::FlyError::Decode)?;
            if let Some(existing) = tokens.iter_mut().find(|item| item.name == token.name) {
                existing.value = token.value.trim().to_string();
            } else {
                tokens.push(DesignToken {
                    name: token.name.clone(),
                    value: token.value.trim().to_string(),
                });
            }
        }
        ThemeCommand::Remove { name } => {
            let before = tokens.len();
            tokens.retain(|item| item.name != *name);
            if tokens.len() == before {
                return Err(crate::FlyError::Decode(format!(
                    "design token `{name}` does not exist"
                )));
            }
        }
    }
    if tokens.len() > MAXIMUM_THEME_TOKENS {
        return Err(crate::FlyError::Decode(format!(
            "flyTheme declares {} tokens, exceeding the maximum of {MAXIMUM_THEME_TOKENS}",
            tokens.len()
        )));
    }
    for token in &tokens {
        validate_token_name(&token.name)
            .and_then(|_| validate_token_value(&token.name, &token.value))
            .map_err(crate::FlyError::Decode)?;
    }
    tokens.sort_by(|left, right| left.name.cmp(&right.name));
    tokens.dedup_by(|next, kept| next.name == kept.name);
    if tokens.is_empty() {
        document.project.extensions.remove(FLY_THEME_FIELD);
    } else {
        let mut map = serde_json::Map::new();
        for token in &tokens {
            map.insert(token.name.clone(), Value::String(token.value.clone()));
        }
        document.project.extensions.insert(
            FLY_THEME_FIELD.to_string(),
            serde_json::json!({ "tokens": Value::Object(map) }),
        );
    }
    Ok(())
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use crate::{
        render_page, EditorCommand, FlyEditor, GrapesJsCodec, PageSelection, RegistrySet,
        RenderPolicy,
    };
    use serde_json::json;

    #[test]
    fn theme_commands_round_trip_through_the_editor() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "id": "home", "component": { "id": "root", "type": "wrapper", "components": [
                { "id": "hero", "type": "section", "style": { "color": "var(--brand)" }, "components": [] }
            ] } }]
        }))
        .expect("document");
        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
        editor
            .apply(EditorCommand::Theme {
                command: ThemeCommand::Upsert {
                    token: DesignToken {
                        name: "brand".to_string(),
                        value: "#0b5fff".to_string(),
                    },
                },
            })
            .expect("upsert");
        editor
            .apply(EditorCommand::Theme {
                command: ThemeCommand::Upsert {
                    token: DesignToken {
                        name: "brand".to_string(),
                        value: "#ff0000".to_string(),
                    },
                },
            })
            .expect("update");
        let page = render_page(
            editor.document(),
            &PageSelection::First,
            &RenderPolicy::default(),
        )
        .expect("render");
        assert!(page.css.contains("--brand:#ff0000"), "css: {}", page.css);
        editor
            .apply(EditorCommand::Theme {
                command: ThemeCommand::Remove {
                    name: "brand".to_string(),
                },
            })
            .expect("remove");
        assert!(decode_theme_tokens(editor.document())
            .expect("tokens")
            .is_empty());
        assert!(
            editor
                .apply(EditorCommand::Theme {
                    command: ThemeCommand::Remove {
                        name: "brand".to_string()
                    },
                })
                .is_err(),
            "removing a missing token must fail"
        );
        editor.undo().expect("undo remove");
        assert_eq!(
            decode_theme_tokens(editor.document())
                .expect("tokens")
                .len(),
            1
        );
    }
}
