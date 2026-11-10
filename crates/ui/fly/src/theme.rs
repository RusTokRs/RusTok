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

use crate::{ComponentChildren, ComponentNode, ProjectDocument};
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

/// Locations where a design token is referenced in CSS declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenUsageLocation {
    ComponentStyle {
        component_id: String,
        property: String,
    },
    StyleRule {
        rule_index: usize,
        property: String,
    },
    ThemeToken {
        token_name: String,
    },
}

/// Recorded usage of a design token in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenUsage {
    pub location: TokenUsageLocation,
    pub snippet: String,
}

/// Replaces exact occurrences of `--{old_name}` with `--{new_name}` in CSS values (e.g. `var(--old_name)`),
/// respecting identifier boundary characters so `--old-name-dark` is not touched.
pub fn replace_token_in_css_value(value: &str, old_name: &str, new_name: &str) -> (String, usize) {
    let target = format!("--{old_name}");
    let replacement = format!("--{new_name}");
    let mut result = String::with_capacity(value.len());
    let mut count = 0;
    let mut last_copied = 0;

    let is_ident_char = |ch: char| -> bool {
        ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch >= '\u{0080}'
    };

    for (idx, _) in value.match_indices(&target) {
        if idx < last_copied {
            continue;
        }
        let before_is_ident = idx > 0 && {
            let ch = value[..idx].chars().next_back().unwrap();
            is_ident_char(ch)
        };
        let after_idx = idx + target.len();
        let after_is_ident = after_idx < value.len() && {
            let ch = value[after_idx..].chars().next().unwrap();
            is_ident_char(ch)
        };

        if !before_is_ident && !after_is_ident {
            result.push_str(&value[last_copied..idx]);
            result.push_str(&replacement);
            count += 1;
            last_copied = after_idx;
        }
    }
    result.push_str(&value[last_copied..]);
    (result, count)
}

fn find_usages_in_opaque_value(
    value: &Value,
    token_name: &str,
    usages: &mut Vec<TokenUsage>,
    fallback_id: &str,
) {
    match value {
        Value::Object(map) => {
            let comp_id = map.get("id").and_then(Value::as_str).unwrap_or(fallback_id);
            if let Some(Value::Object(style_map)) = map.get("style") {
                for (prop, val) in style_map {
                    if let Some(css_val) = val.as_str() {
                        let (_, count) =
                            replace_token_in_css_value(css_val, token_name, token_name);
                        if count > 0 {
                            usages.push(TokenUsage {
                                location: TokenUsageLocation::ComponentStyle {
                                    component_id: comp_id.to_string(),
                                    property: prop.clone(),
                                },
                                snippet: format!("{prop}: {css_val}"),
                            });
                        }
                    }
                }
            }
            if let Some(components) = map.get("components") {
                find_usages_in_opaque_value(components, token_name, usages, comp_id);
            }
        }
        Value::Array(items) => {
            for item in items {
                find_usages_in_opaque_value(item, token_name, usages, fallback_id);
            }
        }
        _ => {}
    }
}

/// Inspect all component styles, style catalog rules, and sibling theme tokens to find usages of `--{token_name}`.
pub fn find_token_usages(document: &ProjectDocument, token_name: &str) -> Vec<TokenUsage> {
    let mut usages = Vec::new();

    // 1. Component inline styles
    document.project.visit_components(|component, _, _| {
        let comp_id = component
            .id
            .clone()
            .unwrap_or_else(|| "anonymous".to_string());
        if let Some(Value::Object(map)) = &component.style {
            for (prop, val) in map {
                if let Some(css_val) = val.as_str() {
                    let (_, count) = replace_token_in_css_value(css_val, token_name, token_name);
                    if count > 0 {
                        usages.push(TokenUsage {
                            location: TokenUsageLocation::ComponentStyle {
                                component_id: comp_id.clone(),
                                property: prop.clone(),
                            },
                            snippet: format!("{prop}: {css_val}"),
                        });
                    }
                }
            }
        }
        if let ComponentChildren::Opaque(val) = &component.components {
            find_usages_in_opaque_value(val, token_name, &mut usages, &comp_id);
        }
    });

    for page in &document.project.pages {
        if let Some(ComponentNode::Opaque(val)) = &page.component {
            find_usages_in_opaque_value(val, token_name, &mut usages, "opaque-page-root");
        }
    }

    // 2. Project styles catalog
    for (idx, rule_val) in document.project.styles.iter().enumerate() {
        if let Some(style_obj) = rule_val.get("style").and_then(Value::as_object) {
            for (prop, val) in style_obj {
                if let Some(css_val) = val.as_str() {
                    let (_, count) = replace_token_in_css_value(css_val, token_name, token_name);
                    if count > 0 {
                        usages.push(TokenUsage {
                            location: TokenUsageLocation::StyleRule {
                                rule_index: idx,
                                property: prop.clone(),
                            },
                            snippet: format!("{prop}: {css_val}"),
                        });
                    }
                }
            }
        }
    }

    // 3. Other tokens in flyTheme
    if let Ok(tokens) = decode_theme_tokens(document) {
        for token in tokens {
            if token.name != token_name {
                let (_, count) = replace_token_in_css_value(&token.value, token_name, token_name);
                if count > 0 {
                    usages.push(TokenUsage {
                        location: TokenUsageLocation::ThemeToken {
                            token_name: token.name.clone(),
                        },
                        snippet: format!("--{}: {}", token.name, token.value),
                    });
                }
            }
        }
    }

    usages
}

/// Rewrites all `var(--{old_name})` occurrences to `var(--{new_name})` across component styles,
/// catalog style rules, and sibling theme tokens in `document`.
pub fn rename_token_references(
    document: &mut ProjectDocument,
    old_name: &str,
    new_name: &str,
) -> usize {
    if old_name == new_name {
        return 0;
    }
    let mut total_replacements = 0;

    // 1. Mutate component styles across all pages
    for page in &mut document.project.pages {
        if let Some(root) = page.component.as_mut() {
            total_replacements += rename_in_component_tree(root, old_name, new_name);
        }
    }

    // 2. Mutate project style catalog rules
    for rule_val in &mut document.project.styles {
        if let Some(style_obj) = rule_val.get_mut("style").and_then(Value::as_object_mut) {
            for val in style_obj.values_mut() {
                if let Some(css_val) = val.as_str() {
                    let (replaced, count) = replace_token_in_css_value(css_val, old_name, new_name);
                    if count > 0 {
                        *val = Value::String(replaced);
                        total_replacements += count;
                    }
                }
            }
        }
    }

    // 3. Mutate sibling theme tokens
    if let Some(theme) = document.project.extensions.get_mut(FLY_THEME_FIELD)
        && let Some(tokens_obj) = theme.get_mut("tokens").and_then(Value::as_object_mut)
    {
        for val in tokens_obj.values_mut() {
            if let Some(css_val) = val.as_str() {
                let (replaced, count) = replace_token_in_css_value(css_val, old_name, new_name);
                if count > 0 {
                    *val = Value::String(replaced);
                    total_replacements += count;
                }
            }
        }
    }

    if total_replacements > 0 {
        document.invalidate_caches();
    }

    total_replacements
}

fn rename_in_component_tree(node: &mut ComponentNode, old_name: &str, new_name: &str) -> usize {
    match node {
        ComponentNode::Object(object) => {
            let mut count = 0;
            if let Some(Value::Object(style_map)) = &mut object.style {
                for val in style_map.values_mut() {
                    if let Some(css_val) = val.as_str() {
                        let (replaced, c) = replace_token_in_css_value(css_val, old_name, new_name);
                        if c > 0 {
                            *val = Value::String(replaced);
                            count += c;
                        }
                    }
                }
            }
            match &mut object.components {
                ComponentChildren::Nodes(children) => {
                    for child in children {
                        count += rename_in_component_tree(child, old_name, new_name);
                    }
                }
                ComponentChildren::Opaque(val) => {
                    count += rename_in_opaque_value(val, old_name, new_name);
                }
            }
            count
        }
        ComponentNode::Opaque(value) => rename_in_opaque_value(value, old_name, new_name),
    }
}

fn rename_in_opaque_value(value: &mut Value, old_name: &str, new_name: &str) -> usize {
    let mut count = 0;
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if key == "style" {
                    if let Value::Object(style_map) = val {
                        for style_val in style_map.values_mut() {
                            if let Some(css_val) = style_val.as_str() {
                                let (replaced, c) =
                                    replace_token_in_css_value(css_val, old_name, new_name);
                                if c > 0 {
                                    *style_val = Value::String(replaced);
                                    count += c;
                                }
                            }
                        }
                    }
                } else if key == "components" {
                    count += rename_in_opaque_value(val, old_name, new_name);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                count += rename_in_opaque_value(item, old_name, new_name);
            }
        }
        _ => {}
    }
    count
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
    /// Rename a token and automatically rewrite all `var(--old_name)` occurrences in styles.
    Rename { old_name: String, new_name: String },
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
        ThemeCommand::Rename { old_name, new_name } => {
            if old_name == new_name {
                return Ok(());
            }
            validate_token_name(new_name).map_err(crate::FlyError::Decode)?;
            let Some(pos) = tokens.iter().position(|t| t.name == *old_name) else {
                return Err(crate::FlyError::Decode(format!(
                    "design token `{old_name}` does not exist"
                )));
            };
            if tokens.iter().any(|t| t.name == *new_name) {
                return Err(crate::FlyError::Decode(format!(
                    "design token `{new_name}` already exists"
                )));
            }
            tokens[pos].name = new_name.clone();
            for item in &mut tokens {
                let (replaced, _) = replace_token_in_css_value(&item.value, old_name, new_name);
                item.value = replaced;
            }
            rename_token_references(document, old_name, new_name);
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
        EditorCommand, FlyEditor, GrapesJsCodec, PageSelection, RegistrySet, RenderPolicy,
        render_page,
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
        assert!(
            decode_theme_tokens(editor.document())
                .expect("tokens")
                .is_empty()
        );
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

    #[test]
    fn theme_rename_command_rewrites_references_and_supports_undo_redo() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyTheme": {
                "tokens": {
                    "brand": "#0b5fff",
                    "accent": "var(--brand)"
                }
            },
            "styles": [
                {
                    "selectors": [{ "name": "cta-btn" }],
                    "style": { "outline": "2px dashed var(--brand)", "color": "var(--brand-dark)" }
                }
            ],
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "hero",
                            "type": "section",
                            "style": {
                                "color": "var(--brand)",
                                "border": "1px solid var(--brand)",
                                "background": "var(--brand-dark)"
                            },
                            "components": []
                        }
                    ]
                }
            }]
        }))
        .expect("document");

        // 1. Usages detection
        let usages = find_token_usages(&document, "brand");
        assert_eq!(
            usages.len(),
            4,
            "Expected 4 usages (2 in component, 1 in catalog, 1 in sibling token)"
        );
        let dark_usages = find_token_usages(&document, "brand-dark");
        assert_eq!(dark_usages.len(), 2, "Expected 2 usages of brand-dark");

        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());

        // 2. Rename brand -> primary
        editor
            .apply(EditorCommand::Theme {
                command: ThemeCommand::Rename {
                    old_name: "brand".to_string(),
                    new_name: "primary".to_string(),
                },
            })
            .expect("rename command succeeds");

        // Verify tokens in theme
        let tokens = decode_theme_tokens(editor.document()).expect("tokens");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].name, "accent");
        assert_eq!(tokens[0].value, "var(--primary)");
        assert_eq!(tokens[1].name, "primary");
        assert_eq!(tokens[1].value, "#0b5fff");

        // Verify component styles updated, while brand-dark was untouched
        let hero = editor.document().component("hero").expect("hero component");
        if let Some(Value::Object(hero_style)) = &hero.style {
            assert_eq!(
                hero_style.get("color").and_then(Value::as_str),
                Some("var(--primary)")
            );
            assert_eq!(
                hero_style.get("border").and_then(Value::as_str),
                Some("1px solid var(--primary)")
            );
            assert_eq!(
                hero_style.get("background").and_then(Value::as_str),
                Some("var(--brand-dark)")
            );
        } else {
            panic!("hero style must be an object");
        }

        // Verify project style catalog updated
        let catalog_style = &editor.document().project.styles[0];
        let style_obj = catalog_style
            .get("style")
            .and_then(Value::as_object)
            .expect("style obj");
        assert_eq!(
            style_obj.get("outline").and_then(Value::as_str),
            Some("2px dashed var(--primary)")
        );
        assert_eq!(
            style_obj.get("color").and_then(Value::as_str),
            Some("var(--brand-dark)")
        );

        // Render page verifies root CSS has --primary
        let page = render_page(
            editor.document(),
            &PageSelection::First,
            &RenderPolicy::default(),
        )
        .expect("render");
        assert!(
            page.css.contains("--primary:#0b5fff"),
            "css must contain --primary"
        );
        assert!(
            !page.css.contains("--brand:#0b5fff"),
            "css must not contain old --brand definition"
        );

        // 3. Test undo
        editor.undo().expect("undo rename succeeds");
        let restored_tokens = decode_theme_tokens(editor.document()).expect("restored tokens");
        assert_eq!(restored_tokens[0].name, "accent");
        assert_eq!(restored_tokens[0].value, "var(--brand)");
        assert_eq!(restored_tokens[1].name, "brand");

        let hero_restored = editor.document().component("hero").expect("hero component");
        if let Some(Value::Object(hero_style)) = &hero_restored.style {
            assert_eq!(
                hero_style.get("color").and_then(Value::as_str),
                Some("var(--brand)")
            );
            assert_eq!(
                hero_style.get("border").and_then(Value::as_str),
                Some("1px solid var(--brand)")
            );
        }

        // 4. Test redo
        editor.redo().expect("redo rename succeeds");
        let redone_tokens = decode_theme_tokens(editor.document()).expect("redone tokens");
        assert_eq!(redone_tokens[1].name, "primary");
        let hero_redone = editor.document().component("hero").expect("hero component");
        if let Some(Value::Object(hero_style)) = &hero_redone.style {
            assert_eq!(
                hero_style.get("color").and_then(Value::as_str),
                Some("var(--primary)")
            );
        }
    }
}
