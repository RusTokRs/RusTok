use std::collections::{BTreeMap, BTreeSet};

use rustok_translation_targets::{
    FieldKey, TranslationPatchIssue, TranslationPatchIssueSeverity, TranslationPatchRequest,
    TranslationPatchValidation, TranslationResourceLifecycle, TranslationResourceSnapshot,
    TranslationStrategy,
};

use crate::{
    TranslationError, TranslationResult,
    glossary::{
        GlossaryConcept, GlossaryMatchKind, GlossaryRecord, GlossaryTermPolicy, GlossaryVariant,
    },
};

pub fn evaluate_patch_qa(
    snapshot: &TranslationResourceSnapshot,
    patch: &TranslationPatchRequest,
    owner_validation: TranslationPatchValidation,
    glossary: Option<&GlossaryRecord>,
) -> TranslationResult<TranslationPatchValidation> {
    owner_validation
        .validate()
        .map_err(|error| TranslationError::InvalidProviderValidation(error.to_string()))?;

    let mut issues = Vec::new();
    if snapshot.summary.lifecycle != TranslationResourceLifecycle::Active {
        issues.push(error_issue(
            None,
            "translation.qa.resource_not_active",
            "translation proposals can be submitted only for active owner resources",
        ));
    }

    let fields = snapshot
        .fields
        .iter()
        .map(|field| (field.descriptor.key.as_str(), field))
        .collect::<BTreeMap<_, _>>();
    let patched_keys = patch
        .fields
        .iter()
        .map(|field| field.key.as_str())
        .collect::<BTreeSet<_>>();

    for field in &snapshot.fields {
        if field.descriptor.required
            && field.descriptor.strategy != TranslationStrategy::Excluded
            && !patched_keys.contains(field.descriptor.key.as_str())
        {
            issues.push(error_issue(
                Some(field.descriptor.key.clone()),
                "translation.qa.required_field_missing",
                "the proposal must include every required translatable field",
            ));
        }
    }

    for field_patch in &patch.fields {
        let Some(field) = fields.get(field_patch.key.as_str()) else {
            issues.push(error_issue(
                Some(field_patch.key.clone()),
                "translation.qa.field_not_declared",
                "the proposal field is not declared by the owner snapshot",
            ));
            continue;
        };

        if field.descriptor.strategy == TranslationStrategy::Excluded {
            issues.push(error_issue(
                Some(field_patch.key.clone()),
                "translation.qa.field_excluded",
                "the owner declares this field as excluded from translation",
            ));
        }
        if field.descriptor.required && field_patch.value.trim().is_empty() {
            issues.push(error_issue(
                Some(field_patch.key.clone()),
                "translation.qa.required_value_empty",
                "a required translation value must not be empty",
            ));
        }
        if let Some(max_characters) = field.descriptor.max_characters {
            let actual = field_patch.value.chars().count();
            if actual > max_characters as usize {
                issues.push(error_issue(
                    Some(field_patch.key.clone()),
                    "translation.qa.max_characters_exceeded",
                    "the translation exceeds the owner-declared character limit",
                ));
            }
        }
        for token in &field.protected_tokens {
            if occurrences(&field.source_value, token) != occurrences(&field_patch.value, token) {
                issues.push(error_issue(
                    Some(field_patch.key.clone()),
                    "translation.qa.protected_token_mismatch",
                    "the translation must preserve every owner-declared protected token exactly",
                ));
                break;
            }
        }
        if field.descriptor.preserves_whitespace
            && whitespace_shape(&field.source_value) != whitespace_shape(&field_patch.value)
        {
            issues.push(error_issue(
                Some(field_patch.key.clone()),
                "translation.qa.whitespace_shape_mismatch",
                "the translation must preserve leading, trailing, and line-break whitespace",
            ));
        }
        if !field.source_value.is_empty()
            && field.source_value == field_patch.value
            && matches!(
                field.descriptor.strategy,
                TranslationStrategy::Translate | TranslationStrategy::TranslateWithPlaceholders
            )
        {
            issues.push(warning_issue(
                Some(field_patch.key.clone()),
                "translation.qa.value_unchanged",
                "the translated value is identical to the source value",
            ));
        }
    }

    if let Some(glossary) = glossary {
        evaluate_glossary_qa(snapshot, patch, glossary, &mut issues)?;
    }

    issues.extend(owner_validation.issues);
    issues.sort_by(|left, right| {
        left.field
            .as_ref()
            .map(FieldKey::as_str)
            .cmp(&right.field.as_ref().map(FieldKey::as_str))
            .then_with(|| severity_order(left.severity).cmp(&severity_order(right.severity)))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    issues.dedup();

    let validation = TranslationPatchValidation {
        accepted: !issues
            .iter()
            .any(|issue| issue.severity == TranslationPatchIssueSeverity::Error),
        issues,
    };
    validation
        .validate()
        .map_err(|error| TranslationError::InvalidProviderValidation(error.to_string()))?;
    crate::observability::record_qa_validation(&validation);
    Ok(validation)
}

fn evaluate_glossary_qa(
    snapshot: &TranslationResourceSnapshot,
    patch: &TranslationPatchRequest,
    glossary: &GlossaryRecord,
    issues: &mut Vec<TranslationPatchIssue>,
) -> TranslationResult<()> {
    if glossary.source_locale != patch.source_locale
        || glossary.target_locale != patch.target_locale
    {
        return Err(TranslationError::GlossaryInvariant(
            "bound glossary locales do not match the proposal locales".to_string(),
        ));
    }
    if !glossary_scope_matches(glossary, &patch.identity) {
        return Ok(());
    }

    let source_fields = snapshot
        .fields
        .iter()
        .map(|field| (field.descriptor.key.as_str(), field.source_value.as_str()))
        .collect::<BTreeMap<_, _>>();
    for field_patch in &patch.fields {
        if glossary
            .scope
            .field_key
            .as_ref()
            .is_some_and(|field_key| field_key.as_str() != field_patch.key.as_str())
        {
            continue;
        }
        let Some(source_value) = source_fields.get(field_patch.key.as_str()) else {
            continue;
        };
        for concept in &glossary.concepts {
            if glossary_concept_matches(source_value, concept) {
                evaluate_concept(concept, field_patch.key.clone(), &field_patch.value, issues);
            }
        }
    }
    Ok(())
}

pub(crate) fn glossary_scope_matches(
    glossary: &GlossaryRecord,
    identity: &rustok_translation_targets::TranslationResourceIdentity,
) -> bool {
    glossary
        .scope
        .owner_slug
        .as_ref()
        .is_none_or(|owner_slug| owner_slug.as_str() == identity.owner_slug.as_str())
        && glossary
            .scope
            .resource_kind
            .as_ref()
            .is_none_or(|resource_kind| resource_kind.as_str() == identity.resource_kind.as_str())
}

pub(crate) fn glossary_concept_matches(source_value: &str, concept: &GlossaryConcept) -> bool {
    term_matches(
        source_value,
        &concept.source_term,
        concept.match_kind,
        concept.case_sensitive,
    )
}

fn evaluate_concept(
    concept: &GlossaryConcept,
    field: FieldKey,
    target_value: &str,
    issues: &mut Vec<TranslationPatchIssue>,
) {
    if concept.variants.iter().any(|variant| {
        variant.policy == GlossaryTermPolicy::Forbidden
            && variant_matches(target_value, variant, concept)
    }) {
        issues.push(error_issue(
            Some(field.clone()),
            "translation.glossary.forbidden_term",
            &format!(
                "the proposal uses a forbidden term for glossary concept `{}`",
                concept.concept_key
            ),
        ));
    }

    if let Some(do_not_translate) = concept
        .variants
        .iter()
        .find(|variant| variant.policy == GlossaryTermPolicy::DoNotTranslate)
    {
        if !variant_matches(target_value, do_not_translate, concept) {
            issues.push(error_issue(
                Some(field),
                "translation.glossary.do_not_translate_changed",
                &format!(
                    "the proposal must preserve glossary concept `{}`",
                    concept.concept_key
                ),
            ));
        }
        return;
    }

    let Some(preferred) = concept
        .variants
        .iter()
        .find(|variant| variant.policy == GlossaryTermPolicy::Preferred)
    else {
        return;
    };
    if variant_matches(target_value, preferred, concept) {
        return;
    }
    if concept.variants.iter().any(|variant| {
        variant.policy == GlossaryTermPolicy::Allowed
            && variant_matches(target_value, variant, concept)
    }) {
        issues.push(warning_issue(
            Some(field),
            "translation.glossary.non_preferred_term",
            &format!(
                "the proposal uses an allowed alternative for glossary concept `{}`",
                concept.concept_key
            ),
        ));
    } else {
        issues.push(error_issue(
            Some(field),
            "translation.glossary.preferred_term_missing",
            &format!(
                "the proposal does not use the preferred term for glossary concept `{}`",
                concept.concept_key
            ),
        ));
    }
}

fn variant_matches(
    target_value: &str,
    variant: &GlossaryVariant,
    concept: &GlossaryConcept,
) -> bool {
    term_matches(
        target_value,
        &variant.value,
        concept.match_kind,
        concept.case_sensitive,
    )
}

fn term_matches(
    value: &str,
    term: &str,
    match_kind: GlossaryMatchKind,
    case_sensitive: bool,
) -> bool {
    let (value, term) = if case_sensitive {
        (value.to_string(), term.to_string())
    } else {
        (value.to_lowercase(), term.to_lowercase())
    };
    match match_kind {
        GlossaryMatchKind::Exact => value == term,
        GlossaryMatchKind::Substring => value.contains(&term),
        GlossaryMatchKind::WholeWord => value.match_indices(&term).any(|(start, matched)| {
            let end = start + matched.len();
            let starts_at_boundary = start == 0
                || value[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|character| !character.is_alphanumeric());
            let ends_at_boundary = end == value.len()
                || value[end..]
                    .chars()
                    .next()
                    .is_none_or(|character| !character.is_alphanumeric());
            starts_at_boundary && ends_at_boundary
        }),
    }
}

fn occurrences(value: &str, token: &str) -> usize {
    value.match_indices(token).count()
}

fn whitespace_shape(value: &str) -> (String, String, Vec<String>) {
    let leading = value
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect();
    let trailing = value
        .chars()
        .rev()
        .take_while(|character| character.is_whitespace())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let line_breaks = value
        .split_inclusive('\n')
        .filter_map(|line| {
            line.strip_suffix("\r\n")
                .map(|_| "\r\n".to_string())
                .or_else(|| line.strip_suffix('\n').map(|_| "\n".to_string()))
        })
        .collect();
    (leading, trailing, line_breaks)
}

fn severity_order(severity: TranslationPatchIssueSeverity) -> u8 {
    match severity {
        TranslationPatchIssueSeverity::Error => 0,
        TranslationPatchIssueSeverity::Warning => 1,
    }
}

fn error_issue(field: Option<FieldKey>, code: &str, message: &str) -> TranslationPatchIssue {
    TranslationPatchIssue {
        field,
        severity: TranslationPatchIssueSeverity::Error,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn warning_issue(field: Option<FieldKey>, code: &str, message: &str) -> TranslationPatchIssue {
    TranslationPatchIssue {
        field,
        severity: TranslationPatchIssueSeverity::Warning,
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::TenantLocale;
    use rustok_translation_targets::{
        FieldKey, OpaqueRevision, OwnerSlug, ResourceId, ResourceKind,
        TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldPatch,
        TranslationFieldSnapshot, TranslationPatchIssueSeverity, TranslationPatchRequest,
        TranslationPatchValidation, TranslationResourceIdentity, TranslationResourceLifecycle,
        TranslationResourceSnapshot, TranslationResourceSummary, TranslationStrategy,
        TranslationValueProfile,
    };
    use uuid::Uuid;

    use super::evaluate_patch_qa;
    use crate::glossary::{
        GlossaryConcept, GlossaryMatchKind, GlossaryRecord, GlossaryScope, GlossaryTermPolicy,
        GlossaryVariant,
    };

    #[test]
    fn deterministic_qa_blocks_required_length_token_whitespace_and_excluded_failures() {
        let snapshot = snapshot();
        let patch = patch(vec![
            field_patch("title", "Hallo"),
            field_patch("template", "{person}"),
            field_patch("internal_code", "translated"),
        ]);

        let validation = evaluate_patch_qa(
            &snapshot,
            &patch,
            TranslationPatchValidation {
                accepted: true,
                issues: Vec::new(),
            },
            None,
        )
        .unwrap();

        assert!(!validation.accepted);
        let codes = validation
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"translation.qa.max_characters_exceeded"));
        assert!(codes.contains(&"translation.qa.required_field_missing"));
        assert!(codes.contains(&"translation.qa.protected_token_mismatch"));
        assert!(codes.contains(&"translation.qa.whitespace_shape_mismatch"));
        assert!(codes.contains(&"translation.qa.field_excluded"));
        assert!(
            validation
                .issues
                .iter()
                .all(|issue| issue.severity == TranslationPatchIssueSeverity::Error)
        );
    }

    #[test]
    fn unchanged_value_is_a_non_blocking_warning() {
        let mut snapshot = snapshot();
        snapshot.fields.truncate(1);
        snapshot.fields[0].descriptor.max_characters = Some(20);
        let validation = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "Hello")]),
            TranslationPatchValidation {
                accepted: true,
                issues: Vec::new(),
            },
            None,
        )
        .unwrap();

        assert!(validation.accepted);
        assert_eq!(validation.issues.len(), 1);
        assert_eq!(
            validation.issues[0].severity,
            TranslationPatchIssueSeverity::Warning
        );
        assert_eq!(validation.issues[0].code, "translation.qa.value_unchanged");
    }

    #[test]
    fn glossary_preferred_allowed_and_missing_terms_have_deterministic_severity() {
        let snapshot = single_field_snapshot("The hero returns");
        let glossary = glossary(vec![concept(
            "hero",
            "hero",
            GlossaryMatchKind::WholeWord,
            false,
            vec![
                variant("Held", GlossaryTermPolicy::Preferred),
                variant("Protagonist", GlossaryTermPolicy::Allowed),
            ],
        )]);

        let preferred = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "Der Held kehrt zurück")]),
            accepted_owner_validation(),
            Some(&glossary),
        )
        .unwrap();
        assert!(preferred.accepted);
        assert!(preferred.issues.is_empty());

        let allowed = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "Der Protagonist kehrt zurück")]),
            accepted_owner_validation(),
            Some(&glossary),
        )
        .unwrap();
        assert!(allowed.accepted);
        assert_eq!(
            allowed.issues[0].code,
            "translation.glossary.non_preferred_term"
        );

        let missing = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "Die Figur kehrt zurück")]),
            accepted_owner_validation(),
            Some(&glossary),
        )
        .unwrap();
        assert!(!missing.accepted);
        assert_eq!(
            missing.issues[0].code,
            "translation.glossary.preferred_term_missing"
        );
    }

    #[test]
    fn glossary_forbidden_and_do_not_translate_terms_block_proposals() {
        let snapshot = single_field_snapshot("RusToK hero");
        let glossary = glossary(vec![
            concept(
                "brand",
                "RusToK",
                GlossaryMatchKind::WholeWord,
                true,
                vec![variant("RusToK", GlossaryTermPolicy::DoNotTranslate)],
            ),
            concept(
                "hero",
                "hero",
                GlossaryMatchKind::WholeWord,
                false,
                vec![variant("Heldchen", GlossaryTermPolicy::Forbidden)],
            ),
        ]);
        let validation = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "Rustok Heldchen")]),
            accepted_owner_validation(),
            Some(&glossary),
        )
        .unwrap();

        assert!(!validation.accepted);
        let codes = validation
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"translation.glossary.do_not_translate_changed"));
        assert!(codes.contains(&"translation.glossary.forbidden_term"));
    }

    #[test]
    fn glossary_scope_whole_word_and_case_sensitivity_are_respected() {
        let snapshot = single_field_snapshot("superhero HERO");
        let mut scoped_elsewhere = glossary(vec![concept(
            "hero",
            "hero",
            GlossaryMatchKind::WholeWord,
            true,
            vec![variant("Held", GlossaryTermPolicy::Preferred)],
        )]);
        scoped_elsewhere.scope.owner_slug = Some(OwnerSlug::new("commerce").unwrap());
        let excluded = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "unchanged")]),
            accepted_owner_validation(),
            Some(&scoped_elsewhere),
        )
        .unwrap();
        assert!(excluded.accepted);
        assert!(excluded.issues.is_empty());

        let matching_scope = glossary(vec![concept(
            "hero",
            "hero",
            GlossaryMatchKind::WholeWord,
            true,
            vec![variant("Held", GlossaryTermPolicy::Preferred)],
        )]);
        let case_sensitive = evaluate_patch_qa(
            &snapshot,
            &patch(vec![field_patch("title", "unchanged")]),
            accepted_owner_validation(),
            Some(&matching_scope),
        )
        .unwrap();
        assert!(case_sensitive.accepted);
        assert!(case_sensitive.issues.is_empty());

        let substring_only_snapshot = single_field_snapshot("superhero");
        let whole_word = evaluate_patch_qa(
            &substring_only_snapshot,
            &patch(vec![field_patch("title", "unchanged")]),
            accepted_owner_validation(),
            Some(&glossary(vec![concept(
                "hero",
                "hero",
                GlossaryMatchKind::WholeWord,
                false,
                vec![variant("Held", GlossaryTermPolicy::Preferred)],
            )])),
        )
        .unwrap();
        assert!(whole_word.accepted);
        assert!(whole_word.issues.is_empty());
    }

    fn snapshot() -> TranslationResourceSnapshot {
        TranslationResourceSnapshot {
            summary: TranslationResourceSummary {
                identity: identity(),
                display_label: "QA fixture".to_string(),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: OpaqueRevision::new("resource-1").unwrap(),
                exact_locales: vec![TenantLocale::new("en").unwrap()],
            },
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            rendered_fallback_locale: None,
            source_revision: OpaqueRevision::new("source-1").unwrap(),
            target_revision: None,
            fields: vec![
                field(
                    "title",
                    "Hello",
                    TranslationStrategy::Translate,
                    true,
                    Some(3),
                    false,
                    Vec::new(),
                ),
                field(
                    "summary",
                    "World",
                    TranslationStrategy::Translate,
                    true,
                    None,
                    false,
                    Vec::new(),
                ),
                field(
                    "template",
                    " {name}\n",
                    TranslationStrategy::TranslateWithPlaceholders,
                    false,
                    None,
                    true,
                    vec!["{name}".to_string()],
                ),
                field(
                    "internal_code",
                    "system",
                    TranslationStrategy::Excluded,
                    false,
                    None,
                    false,
                    Vec::new(),
                ),
            ],
        }
    }

    fn single_field_snapshot(source_value: &str) -> TranslationResourceSnapshot {
        let mut snapshot = snapshot();
        snapshot.fields = vec![field(
            "title",
            source_value,
            TranslationStrategy::Translate,
            true,
            None,
            false,
            Vec::new(),
        )];
        snapshot
    }

    fn glossary(concepts: Vec<GlossaryConcept>) -> GlossaryRecord {
        GlossaryRecord {
            id: Uuid::new_v4(),
            name: "Editorial terms".to_string(),
            description: String::new(),
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            scope: GlossaryScope {
                owner_slug: Some(OwnerSlug::new("content").unwrap()),
                resource_kind: Some(ResourceKind::new("article").unwrap()),
                field_key: Some(FieldKey::new("title").unwrap()),
            },
            is_active: true,
            revision: 2,
            concepts,
        }
    }

    fn concept(
        concept_key: &str,
        source_term: &str,
        match_kind: GlossaryMatchKind,
        case_sensitive: bool,
        variants: Vec<GlossaryVariant>,
    ) -> GlossaryConcept {
        GlossaryConcept {
            concept_key: concept_key.to_string(),
            source_term: source_term.to_string(),
            variants,
            match_kind,
            case_sensitive,
            notes: String::new(),
        }
    }

    fn variant(value: &str, policy: GlossaryTermPolicy) -> GlossaryVariant {
        GlossaryVariant {
            value: value.to_string(),
            policy,
        }
    }

    fn accepted_owner_validation() -> TranslationPatchValidation {
        TranslationPatchValidation {
            accepted: true,
            issues: Vec::new(),
        }
    }

    fn field(
        key: &str,
        source_value: &str,
        strategy: TranslationStrategy,
        required: bool,
        max_characters: Option<u32>,
        preserves_whitespace: bool,
        protected_tokens: Vec<String>,
    ) -> TranslationFieldSnapshot {
        TranslationFieldSnapshot {
            descriptor: TranslationFieldDescriptor {
                key: FieldKey::new(key).unwrap(),
                profile: if strategy == TranslationStrategy::TranslateWithPlaceholders {
                    TranslationValueProfile::TemplateText
                } else {
                    TranslationValueProfile::PlainText
                },
                strategy,
                classification: TranslationDataClassification::Public,
                required,
                ai_export_allowed: strategy != TranslationStrategy::Excluded,
                max_characters,
                preserves_whitespace,
            },
            source_value: source_value.to_string(),
            exact_target_value: None,
            source_hash: format!("sha256:{key}"),
            protected_tokens,
        }
    }

    fn patch(fields: Vec<TranslationFieldPatch>) -> TranslationPatchRequest {
        TranslationPatchRequest {
            identity: identity(),
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            expected_resource_revision: OpaqueRevision::new("resource-1").unwrap(),
            expected_source_revision: OpaqueRevision::new("source-1").unwrap(),
            expected_target_revision: None,
            fields,
            proposal_id: "proposal-1".to_string(),
            approval_receipt_id: "validation-1".to_string(),
        }
    }

    fn field_patch(key: &str, value: &str) -> TranslationFieldPatch {
        TranslationFieldPatch {
            key: FieldKey::new(key).unwrap(),
            value: value.to_string(),
            expected_source_hash: format!("sha256:{key}"),
        }
    }

    fn identity() -> TranslationResourceIdentity {
        TranslationResourceIdentity {
            owner_slug: OwnerSlug::new("content").unwrap(),
            resource_kind: ResourceKind::new("article").unwrap(),
            resource_id: ResourceId::new("qa-1").unwrap(),
            subresource_id: None,
        }
    }
}
