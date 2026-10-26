use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, TenantLocale};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldPatch,
    TranslationFieldSnapshot, TranslationPatchIssue, TranslationPatchIssueSeverity,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationValueProfile,
    validate_translation_apply_context, validate_translation_read_context,
};

#[derive(Clone)]
struct ReferenceProvider {
    state: Arc<Mutex<ReferenceState>>,
}

struct ReferenceState {
    snapshot: TranslationResourceSnapshot,
    revision: u64,
    receipts: BTreeMap<String, (TranslationPatchRequest, TranslationApplicationReceipt)>,
}

impl ReferenceProvider {
    fn new() -> Self {
        let identity = TranslationResourceIdentity {
            owner_slug: OwnerSlug::new("reference").unwrap(),
            resource_kind: ResourceKind::new("article").unwrap(),
            resource_id: ResourceId::new("article-1").unwrap(),
            subresource_id: None,
        };
        let source_locale = TenantLocale::new("en").unwrap();
        let target_locale = TenantLocale::new("fr").unwrap();

        Self {
            state: Arc::new(Mutex::new(ReferenceState {
                snapshot: TranslationResourceSnapshot {
                    summary: TranslationResourceSummary {
                        identity,
                        display_label: "Reference article".to_string(),
                        lifecycle: TranslationResourceLifecycle::Active,
                        resource_revision: revision("resource-1"),
                        exact_locales: vec![source_locale.clone()],
                    },
                    source_locale,
                    target_locale,
                    rendered_fallback_locale: Some(TenantLocale::new("en").unwrap()),
                    source_revision: revision("source-1"),
                    target_revision: None,
                    fields: vec![
                        TranslationFieldSnapshot {
                            descriptor: field("title", true),
                            source_value: "Hello".to_string(),
                            exact_target_value: None,
                            source_hash: "sha256:title-v1".to_string(),
                            protected_tokens: Vec::new(),
                        },
                        TranslationFieldSnapshot {
                            descriptor: field("summary", false),
                            source_value: "Reference summary".to_string(),
                            exact_target_value: None,
                            source_hash: "sha256:summary-v1".to_string(),
                            protected_tokens: Vec::new(),
                        },
                    ],
                },
                revision: 1,
                receipts: BTreeMap::new(),
            })),
        }
    }

    fn snapshot(&self) -> TranslationResourceSnapshot {
        self.state.lock().unwrap().snapshot.clone()
    }

    fn validate_against_state(
        state: &ReferenceState,
        request: &TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        request.validate().map_err(|error| {
            PortError::validation("translation.patch_invalid", error.to_string())
        })?;

        if request.identity != state.snapshot.summary.identity {
            return Err(PortError::not_found(
                "translation.resource_not_found",
                "translation resource was not found",
            ));
        }

        let mut issues = Vec::new();
        if request.expected_resource_revision != state.snapshot.summary.resource_revision {
            issues.push(issue("resource_revision_conflict", None));
        }
        if request.expected_source_revision != state.snapshot.source_revision {
            issues.push(issue("source_revision_conflict", None));
        }
        if request.expected_target_revision != state.snapshot.target_revision {
            issues.push(issue("target_revision_conflict", None));
        }

        for patch in &request.fields {
            match state
                .snapshot
                .fields
                .iter()
                .find(|field| field.descriptor.key == patch.key)
            {
                Some(field) if field.source_hash == patch.expected_source_hash => {}
                Some(_) => issues.push(issue("source_hash_conflict", Some(patch.key.clone()))),
                None => issues.push(issue("unknown_field", Some(patch.key.clone()))),
            }
        }

        Ok(TranslationPatchValidation {
            accepted: issues.is_empty(),
            issues,
        })
    }
}

#[async_trait]
impl TranslationTargetProvider for ReferenceProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new("reference").unwrap(),
            resource_kind: ResourceKind::new("article").unwrap(),
            display_name: "Reference article".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
            ]),
            read_permission_floor: BTreeSet::from(["reference:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["reference:update".to_string()]),
        }
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        request.validate().map_err(|error| {
            PortError::validation("translation.list_invalid", error.to_string())
        })?;
        let state = self.state.lock().unwrap();
        Ok(TranslationResourcePage {
            resources: vec![state.snapshot.summary.clone()],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        if request.source_locale == request.target_locale {
            return Err(PortError::validation(
                "translation.locale_pair_invalid",
                "source and target locale must differ",
            ));
        }
        let state = self.state.lock().unwrap();
        if request.identity != state.snapshot.summary.identity {
            return Err(PortError::not_found(
                "translation.resource_not_found",
                "translation resource was not found",
            ));
        }
        Ok(state.snapshot.clone())
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        let state = self.state.lock().unwrap();
        Self::validate_against_state(&state, &request)
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap().to_string();
        let mut state = self.state.lock().unwrap();

        if let Some((original_request, receipt)) = state.receipts.get(&idempotency_key) {
            return if original_request == &request {
                Ok(receipt.clone())
            } else {
                Err(PortError::conflict(
                    "translation.idempotency_conflict",
                    "idempotency key was already used for a different translation patch",
                ))
            };
        }

        let validation = Self::validate_against_state(&state, &request)?;
        if !validation.accepted {
            return Err(PortError::conflict(
                "translation.revision_conflict",
                "translation patch revisions or source hashes are stale",
            ));
        }

        for patch in &request.fields {
            let field = state
                .snapshot
                .fields
                .iter_mut()
                .find(|field| field.descriptor.key == patch.key)
                .expect("validated field must exist");
            field.exact_target_value = Some(patch.value.clone());
        }

        let target_locale = state.snapshot.target_locale.clone();
        if !state
            .snapshot
            .summary
            .exact_locales
            .contains(&target_locale)
        {
            state.snapshot.summary.exact_locales.push(target_locale);
        }
        state.snapshot.rendered_fallback_locale = None;
        state.revision += 1;
        let next_revision = state.revision;
        state.snapshot.summary.resource_revision = revision(&format!("resource-{next_revision}"));
        state.snapshot.target_revision = Some(revision(&format!("target-{next_revision}")));

        let receipt = TranslationApplicationReceipt {
            provider_receipt_id: format!("receipt-{next_revision}"),
            resource_revision: state.snapshot.summary.resource_revision.clone(),
            target_revision: state.snapshot.target_revision.clone().unwrap(),
            applied_field_keys: request
                .fields
                .iter()
                .map(|field| field.key.clone())
                .collect(),
        };
        state
            .receipts
            .insert(idempotency_key, (request, receipt.clone()));
        Ok(receipt)
    }
}

#[tokio::test]
async fn reference_provider_completes_exact_locale_apply_and_replay() {
    let provider = ReferenceProvider::new();
    let source = provider.snapshot();
    let list = provider
        .list_resources(read_context(), list_request())
        .await
        .unwrap();
    assert_eq!(list.resources.len(), 1);
    assert_eq!(list.resources[0].exact_locales, vec![locale("en")]);

    let before = provider
        .read_resource(read_context(), read_request(&source))
        .await
        .unwrap();
    assert_eq!(before.rendered_fallback_locale, Some(locale("en")));
    assert!(
        before
            .fields
            .iter()
            .all(|field| field.exact_target_value.is_none())
    );

    let patch = patch(&source, "Bonjour");
    let validation = provider
        .validate_patch(read_context(), patch.clone())
        .await
        .unwrap();
    assert!(validation.accepted);

    let first_receipt = provider
        .apply_patch(apply_context("apply-1"), patch.clone())
        .await
        .unwrap();
    let replay_receipt = provider
        .apply_patch(apply_context("apply-1"), patch)
        .await
        .unwrap();
    assert_eq!(first_receipt, replay_receipt);

    let after = provider
        .read_resource(read_context(), read_request(&source))
        .await
        .unwrap();
    assert_eq!(after.rendered_fallback_locale, None);
    assert!(after.summary.exact_locales.contains(&locale("fr")));
    assert_eq!(
        after.fields[0].exact_target_value.as_deref(),
        Some("Bonjour")
    );
}

#[tokio::test]
async fn reference_provider_rejects_stale_patch_without_mutating_target() {
    let provider = ReferenceProvider::new();
    let source = provider.snapshot();
    let mut stale = patch(&source, "Bonjour");
    stale.expected_source_revision = revision("source-stale");

    let validation = provider
        .validate_patch(read_context(), stale.clone())
        .await
        .unwrap();
    assert!(!validation.accepted);
    assert_eq!(validation.issues[0].code, "source_revision_conflict");

    let error = provider
        .apply_patch(apply_context("apply-stale"), stale)
        .await
        .unwrap_err();
    assert_eq!(error.code, "translation.revision_conflict");
    assert!(
        provider
            .snapshot()
            .fields
            .iter()
            .all(|field| field.exact_target_value.is_none())
    );
}

#[tokio::test]
async fn reference_provider_rejects_idempotency_key_reuse_with_new_payload() {
    let provider = ReferenceProvider::new();
    let source = provider.snapshot();
    provider
        .apply_patch(apply_context("apply-1"), patch(&source, "Bonjour"))
        .await
        .unwrap();

    let error = provider
        .apply_patch(apply_context("apply-1"), patch(&source, "Salut"))
        .await
        .unwrap_err();
    assert_eq!(error.code, "translation.idempotency_conflict");
    assert_eq!(
        provider.snapshot().fields[0].exact_target_value.as_deref(),
        Some("Bonjour")
    );
}

fn field(key: &str, required: bool) -> TranslationFieldDescriptor {
    TranslationFieldDescriptor {
        key: FieldKey::new(key).unwrap(),
        profile: TranslationValueProfile::PlainText,
        strategy: TranslationStrategy::Translate,
        classification: TranslationDataClassification::Public,
        required,
        ai_export_allowed: true,
        max_characters: Some(200),
        preserves_whitespace: false,
    }
}

fn issue(code: &str, field: Option<FieldKey>) -> TranslationPatchIssue {
    TranslationPatchIssue {
        field,
        severity: TranslationPatchIssueSeverity::Error,
        code: code.to_string(),
        message: code.replace('_', " "),
    }
}

fn list_request() -> ListTranslationResourcesRequest {
    ListTranslationResourcesRequest {
        source_locale: locale("en"),
        target_locale: locale("fr"),
        cursor: None,
        limit: 50,
    }
}

fn read_request(snapshot: &TranslationResourceSnapshot) -> ReadTranslationResourceRequest {
    ReadTranslationResourceRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: locale("en"),
        target_locale: locale("fr"),
    }
}

fn patch(snapshot: &TranslationResourceSnapshot, value: &str) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: locale("en"),
        target_locale: locale("fr"),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![TranslationFieldPatch {
            key: FieldKey::new("title").unwrap(),
            value: value.to_string(),
            expected_source_hash: "sha256:title-v1".to_string(),
        }],
        proposal_id: "proposal-1".to_string(),
        approval_receipt_id: "approval-1".to_string(),
    }
}

fn read_context() -> PortContext {
    PortContext::new(
        "00000000-0000-0000-0000-000000000001",
        PortActor::service("translation-conformance"),
        "en",
        "translation-conformance-read",
    )
    .with_deadline(Duration::from_secs(1))
}

fn apply_context(idempotency_key: &str) -> PortContext {
    PortContext::new(
        "00000000-0000-0000-0000-000000000001",
        PortActor::service("translation-conformance"),
        "en",
        "translation-conformance-apply",
    )
    .with_deadline(Duration::from_secs(1))
    .with_idempotency_key(idempotency_key)
}

fn locale(value: &str) -> TenantLocale {
    TenantLocale::new(value).unwrap()
}

fn revision(value: &str) -> OpaqueRevision {
    OpaqueRevision::new(value).unwrap()
}
