/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Dictionary definition preparation/recovery. No browser, UI runtime or owner service imports.

use super::write_journal::{
    valid_uuid, JournalScope, JournalState, RecoveryQueryKey, WriteJournal, WriteJournalError,
};
use crate::model::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttributeWriteScope {
    pub tenant_id: String,
    pub actor_id: String,
    pub attribute_id: String,
    pub locale: String,
}
fn canonical_id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}
impl JournalScope for AttributeWriteScope {
    fn valid(&self) -> bool {
        canonical_id(&self.tenant_id)
            && canonical_id(&self.actor_id)
            && canonical_id(&self.attribute_id)
            && rustok_api::TenantLocale::new(self.locale.as_str())
                .is_ok_and(|locale| locale.as_str() == self.locale)
    }
    fn same_identity(&self, other: &Self) -> bool {
        self.tenant_id == other.tenant_id
            && self.actor_id == other.actor_id
            && self.attribute_id == other.attribute_id
    }
    fn storage_key<S: JournalState>(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            S::STORAGE_PREFIX,
            self.tenant_id,
            self.actor_id,
            self.attribute_id
        )
    }
    fn for_locale(&self, locale: String) -> Self {
        Self {
            locale,
            ..self.clone()
        }
    }
    fn attribute_document_id(&self) -> Option<&str> {
        Some(&self.attribute_id)
    }
}

pub(crate) fn state_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn label(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= 255 && !text.contains('\0')
}
fn valid_copy(copy: &AttributeDefinitionCopy) -> bool {
    label(&copy.label)
        && copy
            .help_text
            .as_ref()
            .is_none_or(|text| text.len() <= 4096 && !text.contains('\0'))
        && copy.facet_label.as_ref().is_none_or(|text| label(text))
        && copy.seo_label.as_ref().is_none_or(|text| label(text))
}

pub(crate) fn valid_input(input: &UpdateAttributeDefinitionDraft) -> bool {
    if !canonical_id(&input.attribute_id)
        || input.option_id.as_ref().is_some_and(|id| !canonical_id(id))
        || !state_token(&input.expected_state)
        || !rustok_api::TenantLocale::new(input.locale.as_str())
            .is_ok_and(|locale| locale.as_str() == input.locale)
    {
        return false;
    }
    match (
        input.action,
        &input.option_id,
        &input.attribute_patch,
        &input.option_patch,
    ) {
        (
            AttributeDefinitionAction::Archive | AttributeDefinitionAction::Restore,
            _,
            None,
            None,
        ) => true,
        (AttributeDefinitionAction::Update, None, Some(patch), None) => {
            patch != &AttributeDefinitionPatch::default()
                && patch.translation.as_ref().is_none_or(valid_copy)
                && patch.position.is_none_or(|value| value >= 0)
                && patch.search_weight.is_none_or(|value| value >= 0)
        }
        (AttributeDefinitionAction::Update, Some(_), None, Some(patch)) => {
            patch != &AttributeOptionDefinitionPatch::default()
                && patch.label.as_ref().is_none_or(|text| label(text))
                && patch.position.is_none_or(|value| value >= 0)
        }
        _ => false,
    }
}

pub(crate) fn valid_result(
    input: &UpdateAttributeDefinitionDraft,
    result: &AttributeDefinitionResult,
) -> bool {
    if input.attribute_id != result.attribute_id
        || input.option_id != result.option_id
        || result
            .state_token
            .as_ref()
            .is_some_and(|token| !state_token(token))
    {
        return false;
    }
    let valid_impact = |impact: &AttributeReferenceImpact| {
        impact.references.len() <= 8
            && impact
                .references
                .iter()
                .all(|reference| canonical_id(&reference.resource_id))
    };
    if result.applied {
        result.state_token.is_some() && result.refusal.is_none() && result.impact.is_none()
    } else {
        match result.refusal {
            Some(AttributeDefinitionRefusal::NotFound) => {
                result.state_token.is_none() && result.impact.is_none()
            }
            Some(AttributeDefinitionRefusal::InUse) => {
                input.action == AttributeDefinitionAction::Archive
                    && result.state_token.is_some()
                    && result.impact.as_ref().is_some_and(|impact| {
                        valid_impact(impact)
                            && impact.inspection_complete
                            && (!impact.references.is_empty() || impact.has_more)
                    })
            }
            Some(AttributeDefinitionRefusal::InspectionLimit) => {
                input.action == AttributeDefinitionAction::Archive
                    && result.state_token.is_some()
                    && result
                        .impact
                        .as_ref()
                        .is_some_and(|impact| valid_impact(impact) && !impact.inspection_complete)
            }
            Some(AttributeDefinitionRefusal::Stale) => {
                result.state_token.is_some() && result.impact.is_none()
            }
            Some(AttributeDefinitionRefusal::ParentArchived) => {
                input.action == AttributeDefinitionAction::Restore
                    && input.option_id.is_some()
                    && result.state_token.is_some()
                    && result.impact.is_none()
            }
            None => false,
        }
    }
}

pub(crate) fn terminal(
    input: &UpdateAttributeDefinitionDraft,
    outcome: &AttributeDefinitionOutcome,
) -> bool {
    match outcome {
        AttributeDefinitionOutcome::Decision { result } => valid_result(input, result),
        AttributeDefinitionOutcome::Rejected { code } => code == "validation",
        AttributeDefinitionOutcome::Unknown => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DefinitionPhase {
    Ready,
    Pending {
        key: String,
        input: UpdateAttributeDefinitionDraft,
    },
    Settled {
        key: String,
        input: UpdateAttributeDefinitionDraft,
        outcome: AttributeDefinitionOutcome,
    },
}
impl DefinitionPhase {
    pub fn request(&self) -> Option<(&str, &UpdateAttributeDefinitionDraft)> {
        match self {
            Self::Pending { key, input } | Self::Settled { key, input, .. } => Some((key, input)),
            Self::Ready => None,
        }
    }
}
impl JournalState for DefinitionPhase {
    type Scope = AttributeWriteScope;
    const STORAGE_PREFIX: &'static str = "rustok.product.attribute_definition_attempt";
    const DOCUMENT_QUERY: RecoveryQueryKey = RecoveryQueryKey::AttributeDefinition;
    fn ready() -> Self {
        Self::Ready
    }
    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Settled { .. })
    }
    fn valid(&self) -> bool {
        match self {
            Self::Ready => true,
            Self::Pending { key, input } => valid_uuid(key) && valid_input(input),
            Self::Settled {
                key,
                input,
                outcome,
            } => valid_uuid(key) && valid_input(input) && terminal(input, outcome),
        }
    }
    fn matches_scope(&self, scope: &AttributeWriteScope) -> bool {
        self.request().is_none_or(|(_, input)| {
            input.attribute_id == scope.attribute_id && input.locale == scope.locale
        })
    }
    fn can_follow(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Ready, Self::Pending { .. }) => next.valid(),
            (
                Self::Pending { key, input },
                Self::Settled {
                    key: next_key,
                    input: next_input,
                    outcome,
                },
            ) => key == next_key && input == next_input && terminal(input, outcome),
            _ => false,
        }
    }
}

pub(crate) type DefinitionRecord = WriteJournal<DefinitionPhase>;
impl DefinitionRecord {
    pub fn begin_definition(
        &self,
        input: UpdateAttributeDefinitionDraft,
    ) -> Result<Self, WriteJournalError> {
        if !self.state.is_ready()
            || !valid_input(&input)
            || input.attribute_id != self.scope.attribute_id
        {
            return Err(WriteJournalError::Changed);
        }
        let mut next = self.clone();
        next.scope.locale = input.locale.clone();
        next.state = DefinitionPhase::Pending {
            key: Uuid::new_v4().to_string(),
            input,
        };
        Ok(next)
    }
    pub fn settle_definition(
        &self,
        outcome: AttributeDefinitionOutcome,
    ) -> Result<Self, WriteJournalError> {
        let DefinitionPhase::Pending { key, input } = &self.state else {
            return Err(WriteJournalError::Changed);
        };
        if !terminal(input, &outcome) {
            return Err(WriteJournalError::Invalid);
        }
        let mut next = self.clone();
        next.state = DefinitionPhase::Settled {
            key: key.clone(),
            input: input.clone(),
            outcome,
        };
        Ok(next)
    }
}

/// No synthesized fallback copy. The initial JSON is the exact editable presentation only;
/// identities/storage shape are not accepted patch members. Parsing happens once before IO.
pub(crate) fn presentation_json(
    workspace: &AttributeLifecycleWorkspace,
    option_id: Option<&str>,
) -> Option<String> {
    let attribute = workspace.attributes.first()?;
    let json = if let Some(option_id) = option_id {
        let option = workspace
            .options
            .as_ref()?
            .iter()
            .find(|option| option.id == option_id)?;
        serde_json::to_value(AttributeOptionDefinitionPatch {
            label: option.label.clone(),
            position: Some(option.position),
        })
        .ok()?
    } else {
        serde_json::to_value(AttributeDefinitionPatch {
            translation: attribute.translation.clone(),
            is_filterable: Some(attribute.is_filterable),
            is_searchable: Some(attribute.is_searchable),
            is_sortable: Some(attribute.is_sortable),
            is_comparable: Some(attribute.is_comparable),
            show_on_storefront: Some(attribute.show_on_storefront),
            show_in_admin_grid: Some(attribute.show_in_admin_grid),
            search_weight: Some(attribute.search_weight),
            position: Some(attribute.position),
        })
        .ok()?
    };
    serde_json::to_string_pretty(&json).ok()
}

pub(crate) fn prepare(
    scope: &AttributeWriteScope,
    workspace: &AttributeLifecycleWorkspace,
    option_id: Option<String>,
    action: AttributeDefinitionAction,
    patch_json: &str,
) -> Result<UpdateAttributeDefinitionDraft, ()> {
    if !scope.valid()
        || !valid_workspace(
            workspace,
            &AttributeLifecycleRequest {
                attribute_id: Some(scope.attribute_id.clone()),
                option_id: option_id.clone(),
                include_archived: true,
            },
        )
        || workspace.attributes.len() != 1
        || workspace.attributes[0].id != scope.attribute_id
        || (action == AttributeDefinitionAction::Update && patch_json.len() > 8192)
    {
        return Err(());
    }
    let expected_state = match &option_id {
        Some(id) => workspace
            .options
            .as_ref()
            .and_then(|options| options.iter().find(|option| &option.id == id))
            .map(|option| option.state_token.clone())
            .ok_or(())?,
        None => workspace.attributes[0].state_token.clone(),
    };
    let (attribute_patch, option_patch) = if action == AttributeDefinitionAction::Update {
        let json: serde_json::Value = serde_json::from_str(patch_json).map_err(|_| ())?;
        if !json.is_object()
            || option_id.is_none()
                && json
                    .get("translation")
                    .is_some_and(|copy| !copy.is_null() && !copy.is_object())
        {
            return Err(());
        }
        if option_id.is_none() {
            (Some(serde_json::from_value(json).map_err(|_| ())?), None)
        } else {
            (None, Some(serde_json::from_value(json).map_err(|_| ())?))
        }
    } else {
        (None, None)
    };
    let input = UpdateAttributeDefinitionDraft {
        attribute_id: scope.attribute_id.clone(),
        option_id,
        locale: scope.locale.clone(),
        expected_state,
        action,
        attribute_patch,
        option_patch,
    };
    if valid_input(&input) {
        Ok(input)
    } else {
        Err(())
    }
}

pub(crate) fn result_key(outcome: &AttributeDefinitionOutcome) -> &'static str {
    match outcome {
        AttributeDefinitionOutcome::Decision { result } if result.applied => {
            "product-attributeLifecycle-confirmed"
        }
        AttributeDefinitionOutcome::Decision { result } => match result.refusal {
            Some(AttributeDefinitionRefusal::Stale) => "product-attributeLifecycle-stale",
            Some(AttributeDefinitionRefusal::InUse) => "product-attributeLifecycle-inUse",
            Some(AttributeDefinitionRefusal::InspectionLimit) => "product-attributeLifecycle-limit",
            Some(AttributeDefinitionRefusal::NotFound) => "product-attributeLifecycle-notFound",
            Some(AttributeDefinitionRefusal::ParentArchived) => {
                "product-attributeLifecycle-parentArchived"
            }
            None => "product-attributeLifecycle-unknown",
        },
        AttributeDefinitionOutcome::Rejected { .. } => "product-attributeLifecycle-invalid",
        AttributeDefinitionOutcome::Unknown => "product-attributeLifecycle-unknown",
    }
}

pub(crate) fn inspect(record: &DefinitionRecord) -> Result<String, WriteJournalError> {
    serde_json::to_string_pretty(record).map_err(|_| WriteJournalError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dictionary_and_product_scopes_never_decode_as_each_other() {
        let scope = AttributeWriteScope {
            tenant_id: Uuid::new_v4().to_string(),
            actor_id: Uuid::new_v4().to_string(),
            attribute_id: Uuid::new_v4().to_string(),
            locale: "en".into(),
        };
        let record = DefinitionRecord::new(scope.clone()).unwrap();
        let raw = record.encode().unwrap();
        assert!(raw.contains("attribute_id"));
        assert!(!raw.contains("product_id"));
        assert_eq!(DefinitionRecord::decode(&raw, &scope).unwrap(), record);
        assert_ne!(
            scope.storage_key::<DefinitionPhase>(),
            format!(
                "rustok.product.axis_attempt:{}:{}:{}",
                scope.tenant_id, scope.actor_id, scope.attribute_id
            )
        );
    }
    #[test]
    fn document_selection_never_interprets_url_text_as_the_ui_parent_sentinel() {
        let attribute = Uuid::new_v4().to_string();
        let option = Uuid::new_v4().to_string();
        assert!(valid_document_selection(None, None));
        assert!(valid_document_selection(Some(&attribute), None));
        assert!(valid_document_selection(Some(&attribute), Some(&option)));
        assert!(!valid_document_selection(None, Some(&option)));
        assert!(!valid_document_selection(
            Some(&attribute),
            Some("attribute")
        ));
        assert!(!valid_document_selection(Some(&attribute), Some("")));
        assert!(!valid_document_selection(
            Some(&attribute),
            Some("AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA")
        ));
        assert!(!valid_document_selection(
            Some(&format!(" {attribute}")),
            None
        ));
    }
    #[test]
    fn patch_does_not_accept_identity_or_storage_shape_changes() {
        assert!(serde_json::from_str::<AttributeDefinitionPatch>(r#"{"code":"new"}"#).is_err());
        assert!(serde_json::from_str::<AttributeOptionDefinitionPatch>(r#"{"id":"new"}"#).is_err());
    }
    #[test]
    fn replay_is_scope_bound_keeps_original_locale_and_has_no_fresh_generation_until_terminal() {
        let scope = AttributeWriteScope {
            tenant_id: Uuid::new_v4().to_string(),
            actor_id: Uuid::new_v4().to_string(),
            attribute_id: Uuid::new_v4().to_string(),
            locale: "en".into(),
        };
        let ready = DefinitionRecord::new(scope.clone()).unwrap();
        let input = UpdateAttributeDefinitionDraft {
            attribute_id: scope.attribute_id.clone(),
            option_id: Some(Uuid::new_v4().to_string()),
            locale: "ru".into(),
            expected_state: "a".repeat(64),
            action: AttributeDefinitionAction::Archive,
            attribute_patch: None,
            option_patch: None,
        };
        let pending = ready.begin_definition(input.clone()).unwrap();
        assert!(ready.can_transition_to(&pending));
        assert!(pending.fresh("en".into()).is_err());
        assert!(pending
            .settle_definition(AttributeDefinitionOutcome::Unknown)
            .is_err());
        let restored = DefinitionRecord::decode(&pending.encode().unwrap(), &scope).unwrap();
        assert_eq!(restored, pending);
        assert_eq!(restored.scope.locale, "ru");
        assert_eq!(restored.state.request().unwrap().1, &input);
        let mut foreign = pending.clone();
        foreign.scope.attribute_id = Uuid::new_v4().to_string();
        assert!(foreign.encode().is_err());
        let mut changed = pending.clone();
        if let DefinitionPhase::Pending { key, .. } = &mut changed.state {
            *key = Uuid::new_v4().to_string();
        }
        assert!(!pending.can_restore(&changed));
        let settled = pending
            .settle_definition(AttributeDefinitionOutcome::Decision {
                result: AttributeDefinitionResult {
                    attribute_id: input.attribute_id.clone(),
                    option_id: input.option_id.clone(),
                    applied: false,
                    state_token: Some("b".repeat(64)),
                    refusal: Some(AttributeDefinitionRefusal::Stale),
                    impact: None,
                },
            })
            .unwrap();
        assert!(pending.can_restore(&settled));
        assert_eq!(settled.state.request(), pending.state.request());
        let fresh = settled.fresh("en".into()).unwrap();
        assert_ne!(fresh.intent, pending.intent);
        assert_eq!(fresh.document_intent, pending.document_intent);
        assert!(settled.can_transition_to(&fresh));
        assert!(!pending.can_restore(&fresh));
        assert_eq!(settled.fresh_from_live(&fresh, "ru".into()).unwrap(), fresh);
    }

    #[test]
    fn missing_selected_uuid_stays_unavailable_and_fresh_links_keep_the_exact_option() {
        let id = Uuid::new_v4().to_string();
        let missing = Uuid::new_v4().to_string();
        let scope = AttributeWriteScope {
            tenant_id: Uuid::new_v4().to_string(),
            actor_id: Uuid::new_v4().to_string(),
            attribute_id: id.clone(),
            locale: "en".into(),
        };
        let workspace: AttributeLifecycleWorkspace = serde_json::from_value(serde_json::json!({
            "attributes": [{ "id": id, "code": "color", "valueType": "select", "scope": "both", "isLocalized": false,
                "isFilterable": false, "isSearchable": false, "isSortable": false, "isComparable": false,
                "showOnStorefront": true, "showInAdminGrid": false, "searchWeight": 0, "position": 0,
                "isArchived": false, "translation": null, "stateToken": "a".repeat(64) }],
            "options": [], "impact": null,
        })).unwrap();
        let request = AttributeLifecycleRequest {
            attribute_id: Some(scope.attribute_id.clone()),
            option_id: Some(missing.clone()),
            include_archived: true,
        };
        assert!(valid_workspace(&workspace, &request));
        assert!(presentation_json(&workspace, Some(&missing)).is_none());
        assert!(prepare(
            &scope,
            &workspace,
            Some(missing.clone()),
            AttributeDefinitionAction::Archive,
            "ignored"
        )
        .is_err());
        let options = target_options(
            &workspace,
            &missing,
            "Attribute",
            "Archived",
            "Active",
            "Missing",
        );
        assert!(options
            .iter()
            .any(|(id, label)| id == &missing && label.contains(&missing)));
        let record = DefinitionRecord::new(scope.clone()).unwrap();
        let ready = controls(Some(&record), true, true, false, false, false, true, None);
        assert!(ready.choose_target);
        assert!(ready.refresh);
        assert!(!ready.editable);
        assert!(!ready.recover);
        assert!(!ready.fresh);
        let active = controls(
            Some(&record),
            true,
            true,
            false,
            false,
            false,
            true,
            Some(false),
        );
        assert!(active.archive);
        assert!(!active.restore);
        let archived = controls(
            Some(&record),
            true,
            true,
            false,
            false,
            false,
            true,
            Some(true),
        );
        assert!(!archived.archive);
        assert!(archived.restore);
        let href = selection_href(
            "/modules/product",
            &scope.attribute_id,
            Some(&missing),
            Some(record.document_intent),
        )
        .unwrap();
        assert!(href.contains(&format!("attribute_id={}", scope.attribute_id)));
        assert!(href.contains(&format!("option_id={missing}")));
        assert!(href.contains(&format!("attribute_recovery={}", record.document_intent)));
        assert!(!href.contains("?target_id="));
        let mut fabricated = workspace.clone();
        fabricated.impact = Some(AttributeReferenceImpact {
            references: vec![],
            has_more: false,
            inspection_complete: true,
        });
        assert!(
            !valid_workspace(&fabricated, &request),
            "missing option must not receive the parent's empty impact"
        );
    }
}

pub(crate) fn directory_options(
    workspace: &AttributeLifecycleWorkspace,
    archived: &str,
    active: &str,
) -> Vec<(String, String)> {
    workspace
        .attributes
        .iter()
        .map(|attribute| {
            (
                attribute.id.clone(),
                format!(
                    "{} · {} · {}",
                    attribute
                        .translation
                        .as_ref()
                        .map(|copy| copy.label.as_str())
                        .unwrap_or(&attribute.code),
                    attribute.code,
                    if attribute.is_archived {
                        archived
                    } else {
                        active
                    }
                ),
            )
        })
        .collect()
}
pub(crate) fn target_options(
    workspace: &AttributeLifecycleWorkspace,
    selected: &str,
    attribute_label: &str,
    archived: &str,
    active: &str,
    unavailable: &str,
) -> Vec<(String, String)> {
    let mut options = vec![("attribute".into(), attribute_label.into())];
    options.extend(workspace.options.iter().flatten().map(|option| {
        (
            option.id.clone(),
            format!(
                "{} · {} · {}",
                option.label.as_deref().unwrap_or(&option.code),
                option.code,
                if option.is_archived { archived } else { active }
            ),
        )
    }));
    if selected != "attribute" && !options.iter().any(|(id, _)| id == selected) {
        options.push((selected.into(), format!("{unavailable} · {selected}")));
    }
    options
}
pub(crate) fn selected_option(selected: &str) -> Result<Option<String>, ()> {
    if selected == "attribute" {
        Ok(None)
    } else if canonical_id(selected) {
        Ok(Some(selected.into()))
    } else {
        Err(())
    }
}
pub(crate) fn target_archived(
    workspace: &AttributeLifecycleWorkspace,
    selected: &str,
) -> Option<bool> {
    match selected_option(selected).ok()? {
        None => workspace
            .attributes
            .first()
            .map(|attribute| attribute.is_archived),
        Some(id) => workspace
            .options
            .as_ref()?
            .iter()
            .find(|option| option.id == id)
            .map(|option| option.is_archived),
    }
}
pub(crate) fn selection_href(
    base: &str,
    id: &str,
    option_id: Option<&str>,
    document_intent: Option<Uuid>,
) -> Result<String, ()> {
    if !canonical_id(id)
        || option_id.is_some_and(|id| !canonical_id(id))
        || document_intent.is_some_and(|id| id.is_nil())
        || !base.starts_with('/')
        || base.starts_with("//")
    {
        return Err(());
    }
    Ok(rustok_ui_core::apply_ui_query_pairs(
        &format!("{}/attributes", base.trim_end_matches('/')),
        &[
            (
                rustok_ui_core::AdminQueryKey::AttributeId.as_str(),
                Some(id.into()),
            ),
            (
                rustok_ui_core::AdminQueryKey::OptionId.as_str(),
                option_id.map(str::to_owned),
            ),
            (
                RecoveryQueryKey::AttributeDefinition.as_str(),
                document_intent.map(|id| id.to_string()),
            ),
        ],
    ))
}
pub(crate) fn reference_key(kind: AttributeReferenceKind) -> &'static str {
    match kind {
        AttributeReferenceKind::ProductValue => "product-attributeLifecycle-refProductValue",
        AttributeReferenceKind::VariantValue => "product-attributeLifecycle-refVariantValue",
        AttributeReferenceKind::SchemaBinding => "product-attributeLifecycle-refSchemaBinding",
        AttributeReferenceKind::CategoryBinding => "product-attributeLifecycle-refCategoryBinding",
        AttributeReferenceKind::CloneSnapshot => "product-attributeLifecycle-refCloneSnapshot",
        AttributeReferenceKind::VariantAxis => "product-attributeLifecycle-refVariantAxis",
        AttributeReferenceKind::AllowedOption => "product-attributeLifecycle-refAllowedOption",
        AttributeReferenceKind::AttributeGroup => "product-attributeLifecycle-refAttributeGroup",
        AttributeReferenceKind::ChannelSetting => "product-attributeLifecycle-refChannelSetting",
        AttributeReferenceKind::OptionRule => "product-attributeLifecycle-refOptionRule",
        AttributeReferenceKind::VirtualCategory => "product-attributeLifecycle-refVirtualCategory",
    }
}

pub(crate) fn record_message_key(record: &DefinitionRecord) -> Option<&'static str> {
    match &record.state {
        DefinitionPhase::Ready => None,
        DefinitionPhase::Pending { .. } => Some("product-attributeLifecycle-unknown"),
        DefinitionPhase::Settled { outcome, .. } => Some(result_key(outcome)),
    }
}
pub(crate) fn recorded_impact(record: &DefinitionRecord) -> Option<AttributeReferenceImpact> {
    match &record.state {
        DefinitionPhase::Settled {
            outcome: AttributeDefinitionOutcome::Decision { result },
            ..
        } => result.impact.clone(),
        _ => None,
    }
}
#[derive(Clone, Copy)]
pub(crate) struct DefinitionControls {
    pub editable: bool,
    pub refresh: bool,
    pub choose_target: bool,
    pub archive: bool,
    pub restore: bool,
    pub recover: bool,
    pub fresh: bool,
    pub lock_peers: bool,
}
pub(crate) fn controls(
    record: Option<&DefinitionRecord>,
    has_selection: bool,
    authenticated: bool,
    storage_error: bool,
    busy: bool,
    parent_busy: bool,
    has_parent: bool,
    target_archived: Option<bool>,
) -> DefinitionControls {
    let admitted = authenticated && !storage_error && !busy && !parent_busy;
    let editable_parent =
        admitted && has_parent && record.is_some_and(|record| record.state.is_ready());
    DefinitionControls {
        editable: editable_parent && target_archived.is_some(),
        refresh: admitted
            && (!has_selection || record.is_some_and(|record| record.state.is_ready())),
        choose_target: editable_parent,
        archive: editable_parent && target_archived == Some(false),
        restore: editable_parent && target_archived == Some(true),
        recover: admitted
            && record.is_some_and(|record| matches!(record.state, DefinitionPhase::Pending { .. })),
        fresh: authenticated
            && !busy
            && !parent_busy
            && record.is_some_and(|record| record.state.is_terminal()),
        lock_peers: storage_error
            || has_selection
                && (busy || !authenticated || record.is_none_or(|record| !record.state.is_ready())),
    }
}

pub(crate) fn valid_document_selection(
    attribute_id: Option<&str>,
    option_id: Option<&str>,
) -> bool {
    attribute_id.is_none_or(canonical_id)
        && option_id.is_none_or(|id| attribute_id.is_some() && canonical_id(id))
}
pub(crate) fn valid_read_request(request: &AttributeLifecycleRequest) -> bool {
    valid_document_selection(
        request.attribute_id.as_deref(),
        request.option_id.as_deref(),
    )
}
pub(crate) fn valid_workspace(
    value: &AttributeLifecycleWorkspace,
    request: &AttributeLifecycleRequest,
) -> bool {
    if !valid_read_request(request)
        || value.attributes.len() > 1000
        || value
            .attributes
            .iter()
            .map(|row| &row.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != value.attributes.len()
        || value.attributes.iter().any(|row| {
            !canonical_id(&row.id)
                || !state_token(&row.state_token)
                || !matches!(row.scope.as_str(), "product" | "variant" | "both")
                || !request.include_archived && row.is_archived
        })
    {
        return false;
    }
    if value.impact.as_ref().is_some_and(|impact| {
        impact.references.len() > 8
            || impact
                .references
                .iter()
                .any(|reference| !canonical_id(&reference.resource_id))
    }) {
        return false;
    }
    let Some(attribute_id) = request.attribute_id.as_deref() else {
        return request.option_id.is_none() && value.options.is_none() && value.impact.is_none();
    };
    if value.attributes.len() > 1 || value.attributes.iter().any(|row| row.id != attribute_id) {
        return false;
    }
    if value.attributes.is_empty() {
        return value.options.is_none() && value.impact.is_none();
    }
    let Some(options) = value.options.as_ref() else {
        return false;
    };
    let target_exists = request
        .option_id
        .as_ref()
        .is_none_or(|id| options.iter().any(|row| &row.id == id));
    options.len() <= 1000
        && value.impact.is_some() == target_exists
        && options.iter().all(|row| {
            canonical_id(&row.id)
                && row.attribute_id == attribute_id
                && state_token(&row.state_token)
                && row.attribute_archived == value.attributes[0].is_archived
        })
        && options
            .iter()
            .map(|row| &row.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == options.len()
}
