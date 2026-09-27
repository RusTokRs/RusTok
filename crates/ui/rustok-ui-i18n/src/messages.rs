/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Once, OnceLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use fluent_syntax::ast;
use unic_langid::LanguageIdentifier;

use crate::bundle::{
    FluentCatalog, FluentCatalogBuildReport, build_fluent_catalog_report,
    parse_language_identifier, try_build_fluent_catalog,
};
use crate::error::{BundleBuildError, I18nError, MessageKeyError};
use crate::locale::{MAX_LOCALE_TAG_LEN, canonicalize_language_identifier, locale_candidates};

/// A successfully formatted Fluent value together with the catalog locale that supplied it.
///
/// The provenance is important when a requested locale falls back to a parent or
/// the configured default: callers can update diagnostics, cache metadata, or UI
/// language/direction state without reimplementing lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct ResolvedMessage {
    value: String,
    locale: String,
}

impl ResolvedMessage {
    /// Returns the formatted value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the canonical catalog locale that supplied the value.
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Consumes the result and returns only its formatted value.
    pub fn into_value(self) -> String {
        self.value
    }

    /// Consumes the result and returns `(value, locale)`.
    pub fn into_parts(self) -> (String, String) {
        (self.value, self.locale)
    }
}

/// Ephemeral translator facade over a borrowed `FluentCatalog`.
pub struct UiTranslator<'a> {
    fluent_catalog: &'a FluentCatalog,
    default_locale: &'a str,
}

impl<'a> UiTranslator<'a> {
    pub const fn new(fluent_catalog: &'a FluentCatalog, default_locale: &'a str) -> Self {
        Self {
            fluent_catalog,
            default_locale,
        }
    }

    /// Prepares one locale fallback chain for reuse across multiple message lookups.
    ///
    /// Prefer this in request/render scopes that resolve many keys for the same
    /// effective locale. It avoids reparsing the locale and reallocating the
    /// fallback candidate vector on every lookup.
    pub fn for_locale(&self, locale: Option<&str>) -> UiLocaleTranslator<'a> {
        UiLocaleTranslator::new(self.fluent_catalog, locale, self.default_locale)
    }

    /// Iterates canonical locale identifiers present in this catalog.
    pub fn available_locales(&self) -> impl ExactSizeIterator<Item = &str> {
        self.fluent_catalog.keys().map(String::as_str)
    }

    pub fn try_resolve(&self, locale: Option<&str>, key: &str) -> Result<String, I18nError> {
        try_resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, None)
    }

    pub fn resolve(&self, locale: Option<&str>, key: &str) -> Option<String> {
        resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, None)
    }

    /// Resolves a message and preserves the locale that supplied it.
    pub fn resolve_with_locale(&self, locale: Option<&str>, key: &str) -> Option<ResolvedMessage> {
        resolve_fluent_message_with_locale(
            self.fluent_catalog,
            locale,
            self.default_locale,
            key,
            None,
        )
    }

    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format_message(locale, key, None, fallback)
    }

    pub fn try_format_message<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, args)
    }

    /// Strictly formats a message and preserves the locale that supplied it.
    pub fn try_format_message_with_locale<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        try_resolve_fluent_message_with_locale(
            self.fluent_catalog,
            locale,
            self.default_locale,
            key,
            args,
        )
    }

    /// Strictly resolves and formats a Fluent message attribute.
    pub fn try_format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_attribute(
            self.fluent_catalog,
            locale,
            self.default_locale,
            key,
            attribute,
            args,
        )
    }

    /// Resolves and formats a Fluent message attribute using literal fallback text.
    pub fn format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        resolve_fluent_attribute(
            self.fluent_catalog,
            locale,
            self.default_locale,
            key,
            attribute,
            args,
        )
        .unwrap_or_else(|| fallback.to_string())
    }

    pub fn format_message<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        if let Some(msg) =
            resolve_fluent_message(self.fluent_catalog, locale, self.default_locale, key, args)
        {
            return msg;
        }
        fallback.to_string()
    }
}

/// Translator bound to one effective locale with a precomputed fallback chain.
///
/// Construction performs locale normalization and candidate allocation once;
/// subsequent key lookups reuse the stored candidates. The effective diagnostic
/// locale is borrowed from that same candidate chain, avoiding a duplicate owned
/// `String` per prepared translator.
pub struct UiLocaleTranslator<'a> {
    fluent_catalog: &'a FluentCatalog,
    candidates: Vec<String>,
}

impl<'a> UiLocaleTranslator<'a> {
    pub fn new(
        fluent_catalog: &'a FluentCatalog,
        locale: Option<&str>,
        default_locale: &str,
    ) -> Self {
        Self {
            fluent_catalog,
            candidates: locale_candidates(locale, default_locale),
        }
    }

    /// Returns the normalized fallback candidates reused by this translator.
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    pub fn try_resolve(&self, key: &str) -> Result<String, I18nError> {
        try_resolve_fluent_candidates(
            self.fluent_catalog,
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            None,
        )
    }

    pub fn resolve(&self, key: &str) -> Option<String> {
        resolve_fluent_candidates(self.fluent_catalog, &self.candidates, key, None)
    }

    /// Resolves a message and preserves the locale that supplied it.
    pub fn resolve_with_locale(&self, key: &str) -> Option<ResolvedMessage> {
        resolve_fluent_candidates_with_locale(
            self.fluent_catalog,
            &self.candidates,
            key,
            MessagePart::Value,
            None,
        )
    }

    pub fn t(&self, key: &str, fallback: &str) -> String {
        self.format(key, None, fallback)
    }

    pub fn try_format<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_candidates(
            self.fluent_catalog,
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            args,
        )
    }

    /// Strictly formats a message and preserves the locale that supplied it.
    pub fn try_format_with_locale<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        try_resolve_fluent_candidates_with_locale(
            self.fluent_catalog,
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            MessagePart::Value,
            args,
        )
    }

    /// Strictly resolves and formats a Fluent message attribute.
    pub fn try_format_attribute<'args>(
        &self,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_candidates_with_locale(
            self.fluent_catalog,
            &self.candidates,
            effective_locale(&self.candidates),
            key,
            MessagePart::Attribute(attribute),
            args,
        )
        .map(ResolvedMessage::into_value)
    }

    /// Resolves and formats a Fluent message attribute using literal fallback text.
    pub fn format_attribute<'args>(
        &self,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        resolve_fluent_candidates_with_locale(
            self.fluent_catalog,
            &self.candidates,
            key,
            MessagePart::Attribute(attribute),
            args,
        )
        .map(ResolvedMessage::into_value)
        .unwrap_or_else(|| fallback.to_string())
    }

    pub fn format<'args>(
        &self,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        resolve_fluent_candidates(self.fluent_catalog, &self.candidates, key, args)
            .unwrap_or_else(|| fallback.to_string())
    }
}

/// A fail-closed, fully built message catalog for production startup paths.
///
/// `PreparedUiMessages` is constructed with [`UiMessages::prepare`]. Unlike the
/// lazy [`UiMessages::fluent_catalog`] path, construction rejects malformed
/// locale tags, malformed FTL resources, duplicate normalized locales, an
/// invalid configured default locale, a default locale without an exact
/// normalized catalog entry, and schema mismatches between locales before
/// any lookup can occur.
pub struct PreparedUiMessages {
    default_locale: String,
    fluent_catalog: FluentCatalog,
}

impl std::fmt::Debug for PreparedUiMessages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedUiMessages")
            .field("default_locale", &self.default_locale)
            .field("locales", &self.fluent_catalog.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl PreparedUiMessages {
    /// Returns the validated and normalized default locale tag.
    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    /// Returns the validated Fluent catalog.
    pub const fn fluent_catalog(&self) -> &FluentCatalog {
        &self.fluent_catalog
    }

    /// Iterates canonical locale identifiers present in the validated catalog.
    pub fn available_locales(&self) -> impl ExactSizeIterator<Item = &str> {
        self.fluent_catalog.keys().map(String::as_str)
    }

    /// Borrows this prepared catalog through the common translator facade.
    pub fn translator(&self) -> UiTranslator<'_> {
        UiTranslator::new(&self.fluent_catalog, &self.default_locale)
    }

    /// Prepares one effective locale for repeated lookups against this validated catalog.
    pub fn for_locale(&self, locale: Option<&str>) -> UiLocaleTranslator<'_> {
        UiLocaleTranslator::new(&self.fluent_catalog, locale, &self.default_locale)
    }

    /// Strictly resolves and formats a message without applying literal fallback text.
    pub fn try_format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.translator().try_format_message(locale, key, args)
    }

    /// Strictly formats a message and preserves the locale that supplied it.
    pub fn try_format_with_locale<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        self.translator()
            .try_format_message_with_locale(locale, key, args)
    }

    /// Strictly resolves and formats a Fluent message attribute.
    pub fn try_format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        self.translator()
            .try_format_attribute(locale, key, attribute, args)
    }

    /// Resolves and formats a Fluent message attribute using literal fallback text.
    pub fn format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.translator()
            .format_attribute(locale, key, attribute, args, fallback)
    }

    /// Resolves and formats a message with an explicit literal fallback.
    pub fn format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        self.translator()
            .format_message(locale, key, args, fallback)
    }

    /// Resolves a simple translation key with an explicit literal fallback.
    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.translator().t(locale, key, fallback)
    }
}

/// Message schema representing the external variables required to format a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSchema {
    /// Ordered, deduplicated set of external variable names required by this pattern.
    pub variables: BTreeSet<String>,
}

/// Schema for a complete Fluent message value and each of its attributes.
///
/// Keeping attributes separate prevents an optional/missing translated attribute
/// from changing the variable contract of the message value itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEntrySchema {
    /// Schema for the message value, or `None` for an attribute-only message.
    pub value: Option<MessageSchema>,
    /// Attribute schemas keyed by Fluent attribute identifier.
    pub attributes: BTreeMap<String, MessageSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SchemaNodeId {
    MessageValue(String),
    MessageAttribute(String, String),
    TermValue(String),
    TermAttribute(String, String),
}

impl SchemaNodeId {
    fn display_name(&self) -> String {
        match self {
            Self::MessageValue(message) => message.clone(),
            Self::MessageAttribute(message, attribute) => format!("{message}.{attribute}"),
            Self::TermValue(term) => format!("-{term}"),
            Self::TermAttribute(term, attribute) => format!("-{term}.{attribute}"),
        }
    }
}

#[derive(Debug, Clone)]
struct PatternReference {
    target: SchemaNodeId,
    supplied_variables: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct PatternContract {
    direct_variables: BTreeSet<String>,
    references: Vec<PatternReference>,
}

fn collect_pattern_contract(pattern: &ast::Pattern<&str>, out: &mut PatternContract) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            collect_expression_contract(expression, out);
        }
    }
}

fn collect_expression_contract(expr: &ast::Expression<&str>, out: &mut PatternContract) {
    match expr {
        ast::Expression::Inline(inline) => collect_inline_contract(inline, out),
        ast::Expression::Select { selector, variants } => {
            collect_inline_contract(selector, out);
            for variant in variants {
                collect_pattern_contract(&variant.value, out);
            }
        }
    }
}

fn collect_call_arguments_contract(
    arguments: &ast::CallArguments<&str>,
    out: &mut PatternContract,
) -> BTreeSet<String> {
    for positional in &arguments.positional {
        collect_inline_contract(positional, out);
    }

    let mut supplied = BTreeSet::new();
    for named in &arguments.named {
        supplied.insert(named.name.name.to_string());
        collect_inline_contract(&named.value, out);
    }
    supplied
}

fn collect_inline_contract(inline: &ast::InlineExpression<&str>, out: &mut PatternContract) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            out.direct_variables.insert(id.name.to_string());
        }
        ast::InlineExpression::Placeable { expression } => {
            collect_expression_contract(expression, out);
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            collect_call_arguments_contract(arguments, out);
        }
        ast::InlineExpression::MessageReference { id, attribute } => {
            let target = match attribute {
                Some(attribute) => {
                    SchemaNodeId::MessageAttribute(id.name.to_string(), attribute.name.to_string())
                }
                None => SchemaNodeId::MessageValue(id.name.to_string()),
            };
            out.references.push(PatternReference {
                target,
                supplied_variables: BTreeSet::new(),
            });
        }
        ast::InlineExpression::TermReference {
            id,
            attribute,
            arguments,
        } => {
            let supplied_variables = arguments
                .as_ref()
                .map(|arguments| collect_call_arguments_contract(arguments, out))
                .unwrap_or_default();
            let target = match attribute {
                Some(attribute) => {
                    SchemaNodeId::TermAttribute(id.name.to_string(), attribute.name.to_string())
                }
                None => SchemaNodeId::TermValue(id.name.to_string()),
            };
            out.references.push(PatternReference {
                target,
                supplied_variables,
            });
        }
        _ => {}
    }
}

fn insert_schema_node(
    locale: &str,
    nodes: &mut BTreeMap<SchemaNodeId, PatternContract>,
    id: SchemaNodeId,
    pattern: &ast::Pattern<&str>,
) -> Result<(), BundleBuildError> {
    let mut contract = PatternContract::default();
    collect_pattern_contract(pattern, &mut contract);
    if nodes.insert(id.clone(), contract).is_some() {
        return Err(BundleBuildError::DuplicateEntry {
            locale: locale.to_string(),
            entry: id.display_name(),
        });
    }
    Ok(())
}

fn resolve_node_variables(
    locale: &str,
    node: &SchemaNodeId,
    nodes: &BTreeMap<SchemaNodeId, PatternContract>,
    memo: &mut BTreeMap<SchemaNodeId, BTreeSet<String>>,
    visiting: &mut BTreeSet<SchemaNodeId>,
) -> Result<BTreeSet<String>, BundleBuildError> {
    if let Some(variables) = memo.get(node) {
        return Ok(variables.clone());
    }
    if !visiting.insert(node.clone()) {
        return Err(BundleBuildError::CyclicReference {
            locale: locale.to_string(),
            reference: node.display_name(),
        });
    }

    let Some(contract) = nodes.get(node) else {
        return Err(BundleBuildError::UnresolvedReference {
            locale: locale.to_string(),
            message: node.display_name(),
            reference: node.display_name(),
        });
    };
    let mut variables = contract.direct_variables.clone();

    for reference in &contract.references {
        if !nodes.contains_key(&reference.target) {
            return Err(BundleBuildError::UnresolvedReference {
                locale: locale.to_string(),
                message: node.display_name(),
                reference: reference.target.display_name(),
            });
        }
        let referenced = resolve_node_variables(locale, &reference.target, nodes, memo, visiting)?;
        variables.extend(
            referenced
                .into_iter()
                .filter(|name| !reference.supplied_variables.contains(name)),
        );
    }

    visiting.remove(node);
    memo.insert(node.clone(), variables.clone());
    Ok(variables)
}

fn parse_locale_entry_schemas_for_normalized_locale(
    locale: &str,
    ftl_source: &str,
) -> Result<BTreeMap<String, MessageEntrySchema>, BundleBuildError> {
    let resource = fluent_syntax::parser::parse(ftl_source).map_err(|(_, errors)| {
        BundleBuildError::FluentParse {
            locale: locale.to_string(),
            errors: errors.into_iter().map(|e| format!("{e:?}")).collect(),
        }
    })?;

    let mut nodes = BTreeMap::new();
    let mut messages = BTreeMap::<String, (bool, Vec<String>)>::new();
    let mut seen_entries = BTreeSet::new();

    for entry in &resource.body {
        match entry {
            ast::Entry::Message(message) => {
                let message_id = message.id.name.to_string();
                if !seen_entries.insert(format!("message:{message_id}")) {
                    return Err(BundleBuildError::DuplicateEntry {
                        locale: locale.to_string(),
                        entry: message_id,
                    });
                }
                if let Some(value) = &message.value {
                    insert_schema_node(
                        locale,
                        &mut nodes,
                        SchemaNodeId::MessageValue(message_id.clone()),
                        value,
                    )?;
                }
                let mut attributes = Vec::new();
                for attribute in &message.attributes {
                    let attribute_id = attribute.id.name.to_string();
                    insert_schema_node(
                        locale,
                        &mut nodes,
                        SchemaNodeId::MessageAttribute(message_id.clone(), attribute_id.clone()),
                        &attribute.value,
                    )?;
                    attributes.push(attribute_id);
                }
                messages.insert(message_id, (message.value.is_some(), attributes));
            }
            ast::Entry::Term(term) => {
                let term_id = term.id.name.to_string();
                if !seen_entries.insert(format!("term:{term_id}")) {
                    return Err(BundleBuildError::DuplicateEntry {
                        locale: locale.to_string(),
                        entry: format!("-{term_id}"),
                    });
                }
                insert_schema_node(
                    locale,
                    &mut nodes,
                    SchemaNodeId::TermValue(term_id.clone()),
                    &term.value,
                )?;
                for attribute in &term.attributes {
                    insert_schema_node(
                        locale,
                        &mut nodes,
                        SchemaNodeId::TermAttribute(term_id.clone(), attribute.id.name.to_string()),
                        &attribute.value,
                    )?;
                }
            }
            _ => {}
        }
    }

    let mut memo = BTreeMap::new();
    for node in nodes.keys() {
        resolve_node_variables(locale, node, &nodes, &mut memo, &mut BTreeSet::new())?;
    }

    let mut result = BTreeMap::new();
    for (message, (has_value, attributes)) in messages {
        let value = if has_value {
            Some(MessageSchema {
                variables: resolve_node_variables(
                    locale,
                    &SchemaNodeId::MessageValue(message.clone()),
                    &nodes,
                    &mut memo,
                    &mut BTreeSet::new(),
                )?,
            })
        } else {
            None
        };
        let mut attribute_schemas = BTreeMap::new();
        for attribute in attributes {
            let variables = resolve_node_variables(
                locale,
                &SchemaNodeId::MessageAttribute(message.clone(), attribute.clone()),
                &nodes,
                &mut memo,
                &mut BTreeSet::new(),
            )?;
            attribute_schemas.insert(attribute, MessageSchema { variables });
        }
        result.insert(
            message,
            MessageEntrySchema {
                value,
                attributes: attribute_schemas,
            },
        );
    }
    Ok(result)
}

/// Parses an FTL resource and extracts value/attribute schemas for every message.
pub fn extract_locale_entry_schemas(
    locale: &str,
    ftl_source: &str,
) -> Result<BTreeMap<String, MessageEntrySchema>, BundleBuildError> {
    let locale = parse_language_identifier(locale)?.to_string();
    parse_locale_entry_schemas_for_normalized_locale(&locale, ftl_source)
}

/// Parses an FTL resource and extracts the aggregate variable schema for each message.
///
/// This compatibility API unions value and attribute variables. New validation or
/// tooling that needs to distinguish compound-message parts should use
/// [`extract_locale_entry_schemas`]. Message and term references are resolved
/// transitively, including named term arguments.
pub fn extract_locale_schemas(
    locale: &str,
    ftl_source: &str,
) -> Result<BTreeMap<String, MessageSchema>, BundleBuildError> {
    let entries = extract_locale_entry_schemas(locale, ftl_source)?;
    Ok(entries
        .into_iter()
        .map(|(message, entry)| {
            let mut variables = entry
                .value
                .map(|schema| schema.variables)
                .unwrap_or_default();
            for attribute in entry.attributes.into_values() {
                variables.extend(attribute.variables);
            }
            (message, MessageSchema { variables })
        })
        .collect())
}

/// Validates that message value and attribute schemas match the default locale.
///
/// Missing localized values/attributes are allowed because runtime lookup falls
/// back to the default locale. A non-default locale may not add a message, value,
/// or attribute absent from the default contract. Variables are compared after
/// resolving Fluent message/term references transitively.
pub fn validate_catalog_schemas(
    bundles: &[(&str, &str)],
    default_locale: &str,
) -> Result<(), BundleBuildError> {
    let normalized_default = normalize_default_locale(default_locale)?;
    let mut locale_schemas: BTreeMap<String, BTreeMap<String, MessageEntrySchema>> =
        BTreeMap::new();

    for (locale_tag, ftl_source) in bundles {
        let langid = parse_language_identifier(locale_tag)?;
        let normalized = langid.to_string();
        if locale_schemas.contains_key(&normalized) {
            return Err(BundleBuildError::DuplicateLocale { locale: normalized });
        }
        let schemas = parse_locale_entry_schemas_for_normalized_locale(&normalized, ftl_source)?;
        locale_schemas.insert(normalized, schemas);
    }

    let default_schemas = match locale_schemas.get(&normalized_default) {
        Some(schemas) => schemas,
        None => {
            return Err(BundleBuildError::MissingDefaultLocale {
                locale: normalized_default.clone(),
            });
        }
    };

    for (locale, schemas) in &locale_schemas {
        if locale == &normalized_default {
            continue;
        }

        for message in schemas.keys() {
            if !default_schemas.contains_key(message) {
                return Err(BundleBuildError::ExtraMessage {
                    locale: locale.clone(),
                    message: message.clone(),
                });
            }
        }

        for (message, default_schema) in default_schemas {
            let Some(locale_schema) = schemas.get(message) else {
                continue;
            };

            match (&default_schema.value, &locale_schema.value) {
                (None, Some(_)) => {
                    return Err(BundleBuildError::ExtraMessageValue {
                        locale: locale.clone(),
                        message: message.clone(),
                    });
                }
                (Some(expected), Some(actual)) if actual.variables != expected.variables => {
                    return Err(BundleBuildError::MessageSchemaMismatch {
                        locale: locale.clone(),
                        message: message.clone(),
                        expected: expected.variables.iter().cloned().collect(),
                        actual: actual.variables.iter().cloned().collect(),
                    });
                }
                _ => {}
            }

            for attribute in locale_schema.attributes.keys() {
                if !default_schema.attributes.contains_key(attribute) {
                    return Err(BundleBuildError::ExtraMessageAttribute {
                        locale: locale.clone(),
                        message: message.clone(),
                        attribute: attribute.clone(),
                    });
                }
            }

            for (attribute, expected) in &default_schema.attributes {
                if let Some(actual) = locale_schema.attributes.get(attribute)
                    && actual.variables != expected.variables
                {
                    return Err(BundleBuildError::MessageAttributeSchemaMismatch {
                        locale: locale.clone(),
                        message: message.clone(),
                        attribute: attribute.clone(),
                        expected: expected.variables.iter().cloned().collect(),
                        actual: actual.variables.iter().cloned().collect(),
                    });
                }
            }
        }
    }

    Ok(())
}

/// Primary thread-safe (`Send + Sync`) container for module-owned UI translations.
///
/// Stores compile-time embedded message bundles and lazily initializes one
/// lenient catalog build report on first message resolution or diagnostic access.
/// The cached report owns both the usable concurrent Fluent catalog and typed
/// skipped-entry/configuration diagnostics. Use [`UiMessages::prepare`] when
/// startup must fail closed on catalog/configuration errors.
pub struct UiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
    fluent_catalog: OnceLock<FluentCatalogBuildReport>,
    diagnostics_logged: Once,
}

impl UiMessages {
    /// Creates a new `UiMessages` instance with static bundle pairs.
    pub const fn new(
        default_locale: &'static str,
        bundles: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            default_locale,
            bundles,
            fluent_catalog: OnceLock::new(),
            diagnostics_logged: Once::new(),
        }
    }

    /// Strictly validates the configured default locale and every embedded bundle.
    ///
    /// The normalized default locale must also have an exact catalog entry, matching
    /// the `@rustok/next-fluent` configuration invariant that `defaultLocale` is one
    /// of the configured locales. Additionally, cross-locale message schemas are
    /// validated for variable parity against the default locale.
    ///
    /// This is intended for tests and CI. Production startup code that wants to
    /// validate once and reuse the exact validated catalog should call [`Self::prepare`].
    pub fn validate(&self) -> Result<(), BundleBuildError> {
        let default_locale = normalize_default_locale(self.default_locale)?;
        let fluent_catalog = try_build_fluent_catalog(self.bundles)?;
        ensure_default_locale_present(&fluent_catalog, &default_locale)?;
        validate_catalog_schemas(self.bundles, &default_locale)
    }

    /// Builds a fail-closed catalog once and returns an owned prepared runtime.
    ///
    /// This avoids the validate-then-rebuild pattern: the returned object serves
    /// lookups from the same strict catalog that passed construction. The normalized
    /// default locale must be present in that exact catalog, and message schemas
    /// must match across all locales.
    pub fn prepare(&self) -> Result<PreparedUiMessages, BundleBuildError> {
        let default_locale = normalize_default_locale(self.default_locale)?;
        let fluent_catalog = try_build_fluent_catalog(self.bundles)?;
        ensure_default_locale_present(&fluent_catalog, &default_locale)?;
        validate_catalog_schemas(self.bundles, &default_locale)?;
        Ok(PreparedUiMessages {
            default_locale,
            fluent_catalog,
        })
    }

    fn fluent_catalog_report(&self) -> &FluentCatalogBuildReport {
        let report = self.fluent_catalog.get_or_init(|| {
            let mut report = build_fluent_catalog_report(self.bundles);

            match normalize_default_locale(self.default_locale) {
                Ok(default_locale) => {
                    if let Err(error) =
                        ensure_default_locale_present(report.catalog(), &default_locale)
                    {
                        report.push_diagnostic(error);
                    } else if report.is_clean()
                        && let Err(error) = validate_catalog_schemas(self.bundles, &default_locale)
                    {
                        // Lenient rendering remains available, but startup health can
                        // now observe schema/reference defects without a second build.
                        report.push_diagnostic(error);
                    }
                }
                Err(error) => {
                    report.push_diagnostic(error);
                }
            }

            report
        });

        self.diagnostics_logged.call_once(|| {
            for diagnostic in report.diagnostics() {
                match diagnostic {
                    BundleBuildError::LocaleTooLong { length, max_len } => {
                        tracing::error!(length, max_len, "Skipping oversized Fluent locale");
                    }
                    BundleBuildError::InvalidLocale { locale, .. } => {
                        tracing::error!(%diagnostic, locale = locale.as_str(), "Skipping invalid Fluent locale");
                    }
                    BundleBuildError::DuplicateLocale { locale } => {
                        tracing::error!(%diagnostic, locale = locale.as_str(), "Skipping duplicate normalized Fluent locale");
                    }
                    BundleBuildError::MissingDefaultLocale { locale } => {
                        tracing::error!(%diagnostic, default_locale = locale.as_str(), "Configured Fluent default locale is absent from the usable catalog");
                    }
                    BundleBuildError::InvalidDefaultLocale { locale, .. } => {
                        tracing::error!(%diagnostic, default_locale = locale.as_str(), "Configured Fluent default locale is invalid");
                    }
                    _ => {
                        tracing::error!(%diagnostic, "Skipping invalid Fluent bundle entry");
                    }
                }
            }
        });

        report
    }

    /// Accesses the underlying lazily initialized lenient `FluentCatalog`.
    ///
    /// Invalid bundle entries are logged and skipped by this convenience path.
    /// The same one-time initialization also retains typed entry/configuration
    /// diagnostics, available through [`Self::initialization_diagnostics`]. Use
    /// [`Self::prepare`] when catalog construction errors must fail closed instead.
    pub fn fluent_catalog(&self) -> &FluentCatalog {
        self.fluent_catalog_report().catalog()
    }

    /// Iterates canonical locale identifiers accepted by lenient initialization.
    pub fn available_locales(&self) -> impl ExactSizeIterator<Item = &str> {
        self.fluent_catalog().keys().map(String::as_str)
    }

    /// Returns typed diagnostics retained by the lazy lenient initialization.
    ///
    /// Diagnostics include both skipped catalog entries and invalid/missing default
    /// locale configuration. Calling this before the first lookup triggers the same
    /// one-time `OnceLock` initialization used by [`Self::fluent_catalog`]; it never
    /// rebuilds the catalog solely to recover diagnostics. The returned slice remains
    /// stable for the lifetime of this `UiMessages` value.
    pub fn initialization_diagnostics(&self) -> &[BundleBuildError] {
        self.fluent_catalog_report().diagnostics()
    }

    /// Prepares one effective locale for repeated lookups through the lazy catalog.
    pub fn for_locale(&self, locale: Option<&str>) -> UiLocaleTranslator<'_> {
        UiLocaleTranslator::new(self.fluent_catalog(), locale, self.default_locale)
    }

    /// Resolves a simple translation key for the specified locale, falling back if not found.
    pub fn t(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.format(locale, key, None, fallback)
    }

    /// Resolves a simple translation key for the specified locale (alias for `t`).
    pub fn t_for_locale(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        self.t(locale, key, fallback)
    }

    /// Strictly resolves and formats a message without applying literal fallback text.
    ///
    /// This method is strict about lookup/formatting but uses the lazily initialized
    /// lenient catalog for backward compatibility. Use [`Self::prepare`] when
    /// bundle construction itself must also be fail-closed.
    pub fn try_format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_message(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            args,
        )
    }

    /// Strictly formats a message and preserves the locale that supplied it.
    pub fn try_format_with_locale<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<ResolvedMessage, I18nError> {
        try_resolve_fluent_message_with_locale(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            args,
        )
    }

    /// Strictly resolves and formats a Fluent message attribute.
    pub fn try_format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
    ) -> Result<String, I18nError> {
        try_resolve_fluent_attribute(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            attribute,
            args,
        )
    }

    /// Resolves and formats a Fluent message attribute with an explicit fallback.
    pub fn format_attribute<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        attribute: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        resolve_fluent_attribute(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            attribute,
            args,
        )
        .unwrap_or_else(|| fallback.to_string())
    }

    /// Resolves and formats a message with parameters.
    pub fn format<'args>(
        &self,
        locale: Option<&str>,
        key: &str,
        args: Option<&FluentArgs<'args>>,
        fallback: &str,
    ) -> String {
        if let Some(msg) = resolve_fluent_message(
            self.fluent_catalog(),
            locale,
            self.default_locale,
            key,
            args,
        ) {
            return msg;
        }

        fallback.to_string()
    }
}

pub(crate) fn normalize_default_locale(default_locale: &str) -> Result<String, BundleBuildError> {
    if default_locale.len() > MAX_LOCALE_TAG_LEN {
        return Err(BundleBuildError::LocaleTooLong {
            length: default_locale.len(),
            max_len: MAX_LOCALE_TAG_LEN,
        });
    }

    let trimmed = default_locale.trim();
    let normalized = trimmed.replace('_', "-");
    normalized
        .parse::<LanguageIdentifier>()
        .map(canonicalize_language_identifier)
        .map(|langid| langid.to_string())
        .map_err(|source| BundleBuildError::InvalidDefaultLocale {
            locale: default_locale.to_string(),
            source,
        })
}

fn ensure_default_locale_present(
    fluent_catalog: &FluentCatalog,
    default_locale: &str,
) -> Result<(), BundleBuildError> {
    if fluent_catalog.contains_key(default_locale) {
        Ok(())
    } else {
        Err(BundleBuildError::MissingDefaultLocale {
            locale: default_locale.to_string(),
        })
    }
}

#[inline]
pub(crate) fn effective_locale(candidates: &[String]) -> &str {
    candidates.first().map(String::as_str).unwrap_or("en")
}

const STACK_KEY_BUF_SIZE: usize = 128;

/// Executes a closure with a kebab-case representation of `key`.
///
/// If `key` contains '.', replaces '.' with '-' using a fixed stack buffer for
/// keys <= 128 bytes, avoiding heap allocation on the normal stack path. A safe
/// UTF-8 validation guards the stack slice; an unexpected validation failure
/// falls back to the ordinary allocating replacement instead of invoking `unsafe`.
#[inline]
pub fn with_kebab_key<R>(key: &str, f: impl FnOnce(&str) -> R) -> R {
    if !key.contains('.') {
        return f(key);
    }

    if key.len() <= STACK_KEY_BUF_SIZE {
        let mut buf = [0u8; STACK_KEY_BUF_SIZE];
        let bytes = key.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            buf[i] = if b == b'.' { b'-' } else { b };
        }

        let Ok(kebab) = std::str::from_utf8(&buf[..key.len()]) else {
            let kebab = key.replace('.', "-");
            return f(&kebab);
        };
        return f(kebab);
    }

    let kebab = key.replace('.', "-");
    f(&kebab)
}

/// Maximum supported byte length for message keys.
pub const MAX_MESSAGE_KEY_LEN: usize = 256;

/// Validates an application message key against Fluent identifier syntax.
///
/// Dots are accepted as the documented application alias for Fluent hyphens.
/// All other characters must match Fluent's ASCII message-id grammar.
pub fn validate_message_key(key: &str) -> Result<(), I18nError> {
    validate_identifier(key, true).map_err(|reason| I18nError::InvalidMessageKey {
        key: diagnostic_identifier(key),
        reason,
    })
}

/// Validates a Fluent message attribute identifier.
pub fn validate_message_attribute(attribute: &str) -> Result<(), I18nError> {
    validate_identifier(attribute, false).map_err(|reason| I18nError::InvalidMessageAttribute {
        attribute: diagnostic_identifier(attribute),
        reason,
    })
}

fn validate_identifier(value: &str, allow_dots: bool) -> Result<(), MessageKeyError> {
    if value.is_empty() {
        return Err(MessageKeyError::Empty);
    }
    if value.len() > MAX_MESSAGE_KEY_LEN {
        return Err(MessageKeyError::TooLong {
            length: value.len(),
            max_len: MAX_MESSAGE_KEY_LEN,
        });
    }
    if value.chars().any(char::is_control) {
        return Err(MessageKeyError::InvalidCharacters);
    }

    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
        || !bytes.all(|byte| {
            byte.is_ascii_alphanumeric()
                || byte == b'_'
                || byte == b'-'
                || (allow_dots && byte == b'.')
        })
    {
        return Err(MessageKeyError::InvalidSyntax);
    }
    Ok(())
}

fn diagnostic_identifier(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        truncate_for_diagnostic(value, 128)
    }
}

fn truncate_for_diagnostic(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let mut truncated = String::with_capacity(max_bytes + 3);
        for c in s.chars() {
            if truncated.len() + c.len_utf8() > max_bytes {
                break;
            }
            truncated.push(c);
        }
        truncated.push_str("...");
        truncated
    }
}

pub(crate) type ConcurrentFluentBundle = FluentBundle<FluentResource>;

#[derive(Clone, Copy)]
pub(crate) enum MessagePart<'a> {
    Value,
    Attribute(&'a str),
}

impl MessagePart<'_> {
    fn missing_error(&self, effective_locale: &str, key: &str) -> I18nError {
        match self {
            Self::Value => I18nError::MessageNotFound {
                locale: effective_locale.to_string(),
                key: key.to_string(),
            },
            Self::Attribute(attribute) => I18nError::AttributeNotFound {
                locale: effective_locale.to_string(),
                key: key.to_string(),
                attribute: (*attribute).to_string(),
            },
        }
    }

    fn diagnostic_key(&self, key: &str) -> String {
        match self {
            Self::Value => key.to_string(),
            Self::Attribute(attribute) => format!("{key}.{attribute}"),
        }
    }
}

enum LookupResult {
    Found {
        value: String,
        candidate_index: usize,
    },
    Missing,
    Failed(I18nError),
}

fn lookup_fluent_candidates_with<'bundle, 'args>(
    candidates: &[String],
    key: &str,
    part: MessagePart<'_>,
    args: Option<&FluentArgs<'args>>,
    mut bundle_for_locale: impl FnMut(&str) -> Option<&'bundle ConcurrentFluentBundle>,
) -> LookupResult {
    if let Err(error) = validate_message_key(key) {
        return LookupResult::Failed(error);
    }
    if let MessagePart::Attribute(attribute) = part
        && let Err(error) = validate_message_attribute(attribute)
    {
        return LookupResult::Failed(error);
    }

    with_kebab_key(key, |lookup_key| {
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            let Some(bundle) = bundle_for_locale(candidate.as_str()) else {
                continue;
            };
            let Some(message) = bundle.get_message(lookup_key) else {
                continue;
            };
            let pattern = match part {
                MessagePart::Value => message.value(),
                MessagePart::Attribute(attribute) => message
                    .get_attribute(attribute)
                    .map(|attribute| attribute.value()),
            };
            let Some(pattern) = pattern else {
                continue;
            };

            let mut errors = vec![];
            let formatted = bundle.format_pattern(pattern, args, &mut errors);
            if !errors.is_empty() {
                return LookupResult::Failed(I18nError::FormattingFailed {
                    locale: candidate.clone(),
                    key: part.diagnostic_key(key),
                    errors,
                });
            }
            return LookupResult::Found {
                value: formatted.to_string(),
                candidate_index,
            };
        }

        LookupResult::Missing
    })
}

/// Strictly resolves a message against the `FluentCatalog` using the locale
/// fallback candidate chain.
pub fn try_resolve_fluent_message<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<String, I18nError> {
    try_resolve_fluent_message_with_locale(catalog, locale, default_locale, key, args)
        .map(ResolvedMessage::into_value)
}

/// Strictly resolves a message and preserves the canonical source locale.
pub fn try_resolve_fluent_message_with_locale<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<ResolvedMessage, I18nError> {
    let candidates = locale_candidates(locale, default_locale);
    try_resolve_fluent_candidates_with_locale(
        catalog,
        &candidates,
        effective_locale(&candidates),
        key,
        MessagePart::Value,
        args,
    )
}

/// Strictly resolves a Fluent message attribute.
pub fn try_resolve_fluent_attribute<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    attribute: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<String, I18nError> {
    try_resolve_fluent_attribute_with_locale(catalog, locale, default_locale, key, attribute, args)
        .map(ResolvedMessage::into_value)
}

/// Strictly resolves a Fluent message attribute and preserves its source locale.
pub fn try_resolve_fluent_attribute_with_locale<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    attribute: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<ResolvedMessage, I18nError> {
    let candidates = locale_candidates(locale, default_locale);
    try_resolve_fluent_candidates_with_locale(
        catalog,
        &candidates,
        effective_locale(&candidates),
        key,
        MessagePart::Attribute(attribute),
        args,
    )
}

fn try_resolve_fluent_candidates<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    effective_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Result<String, I18nError> {
    try_resolve_fluent_candidates_with_locale(
        catalog,
        candidates,
        effective_locale,
        key,
        MessagePart::Value,
        args,
    )
    .map(ResolvedMessage::into_value)
}

fn try_resolve_fluent_candidates_with_locale<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    effective_locale: &str,
    key: &str,
    part: MessagePart<'_>,
    args: Option<&FluentArgs<'args>>,
) -> Result<ResolvedMessage, I18nError> {
    try_resolve_candidates_with_provider(
        candidates,
        effective_locale,
        key,
        part,
        args,
        |candidate| catalog.get(candidate),
    )
}

pub(crate) fn try_resolve_candidates_with_provider<'bundle, 'args>(
    candidates: &[String],
    effective_locale: &str,
    key: &str,
    part: MessagePart<'_>,
    args: Option<&FluentArgs<'args>>,
    bundle_for_locale: impl FnMut(&str) -> Option<&'bundle ConcurrentFluentBundle>,
) -> Result<ResolvedMessage, I18nError> {
    match lookup_fluent_candidates_with(candidates, key, part, args, bundle_for_locale) {
        LookupResult::Found {
            value,
            candidate_index,
        } => Ok(ResolvedMessage {
            value,
            locale: candidates[candidate_index].clone(),
        }),
        LookupResult::Missing => Err(part.missing_error(effective_locale, key)),
        LookupResult::Failed(error) => Err(error),
    }
}

/// Resolves a message using lenient UI semantics.
///
/// Missing messages return `None`. Formatting failures are logged and also
/// return `None`, allowing the caller to use its explicit literal fallback
/// instead of rendering a partially formatted message.
pub fn resolve_fluent_message<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    resolve_fluent_message_with_locale(catalog, locale, default_locale, key, args)
        .map(ResolvedMessage::into_value)
}

/// Resolves a message leniently and preserves the canonical source locale.
pub fn resolve_fluent_message_with_locale<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<ResolvedMessage> {
    let candidates = locale_candidates(locale, default_locale);
    resolve_fluent_candidates_with_locale(catalog, &candidates, key, MessagePart::Value, args)
}

/// Resolves a Fluent message attribute using lenient UI semantics.
pub fn resolve_fluent_attribute<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    attribute: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    resolve_fluent_attribute_with_locale(catalog, locale, default_locale, key, attribute, args)
        .map(ResolvedMessage::into_value)
}

/// Resolves an attribute leniently and preserves the canonical source locale.
pub fn resolve_fluent_attribute_with_locale<'args>(
    catalog: &FluentCatalog,
    locale: Option<&str>,
    default_locale: &str,
    key: &str,
    attribute: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<ResolvedMessage> {
    let candidates = locale_candidates(locale, default_locale);
    resolve_fluent_candidates_with_locale(
        catalog,
        &candidates,
        key,
        MessagePart::Attribute(attribute),
        args,
    )
}

fn resolve_fluent_candidates<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    key: &str,
    args: Option<&FluentArgs<'args>>,
) -> Option<String> {
    resolve_fluent_candidates_with_locale(catalog, candidates, key, MessagePart::Value, args)
        .map(ResolvedMessage::into_value)
}

fn resolve_fluent_candidates_with_locale<'args>(
    catalog: &FluentCatalog,
    candidates: &[String],
    key: &str,
    part: MessagePart<'_>,
    args: Option<&FluentArgs<'args>>,
) -> Option<ResolvedMessage> {
    resolve_candidates_with_provider(candidates, key, part, args, |candidate| {
        catalog.get(candidate)
    })
}

pub(crate) fn resolve_candidates_with_provider<'bundle, 'args>(
    candidates: &[String],
    key: &str,
    part: MessagePart<'_>,
    args: Option<&FluentArgs<'args>>,
    bundle_for_locale: impl FnMut(&str) -> Option<&'bundle ConcurrentFluentBundle>,
) -> Option<ResolvedMessage> {
    match lookup_fluent_candidates_with(candidates, key, part, args, bundle_for_locale) {
        LookupResult::Found {
            value,
            candidate_index,
        } => Some(ResolvedMessage {
            value,
            locale: candidates[candidate_index].clone(),
        }),
        LookupResult::Missing => None,
        LookupResult::Failed(error) => {
            tracing::warn!(
                %error,
                key = %truncate_for_diagnostic(&part.diagnostic_key(key), 128),
                "Fluent message resolution failed"
            );
            None
        }
    }
}
