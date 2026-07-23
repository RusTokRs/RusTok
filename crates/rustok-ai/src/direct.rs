#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use alloy::ScriptRegistry;
use alloy::utils::{dynamic_to_json, json_to_dynamic};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use rustok_ai_alloy::AlloyOperation;
use rustok_ai_content::{
    BLOG_DRAFT_TASK_SLUG, BLOG_DRAFT_TOOL_NAME, GeneratedBlogDraft, GeneratedModerationDecision,
    blog_draft_must_remain_unpublished, validate_blog_draft_payload, validate_moderation_decision,
};
use rustok_ai_product::{
    GeneratedProductAttributes, GeneratedProductCopy, PRODUCT_COPY_TASK_SLUG,
    PRODUCT_COPY_TOOL_NAME, validate_product_attributes_payload, validate_product_copy_payload,
};
use rustok_api::{PortActor, PortContext};
use rustok_blog::{CreatePostInput, PostService, UpdatePostInput};
use rustok_core::infer_user_role_from_permissions;
use rustok_mcp::alloy_tools::{AlloyMcpState, ValidateScriptRequest, alloy_validate_script};
use rustok_media::{MediaService, UploadInput, UpsertTranslationInput};
use rustok_product::CatalogService;
use rustok_product::dto::{ProductTranslationInput, UpdateProductInput};
use rustok_storage::StorageRuntime;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::engine::InferenceEngine;
use crate::model::{
    AiAlloyTaskInput, AiBlogDraftTaskInput, AiContentModerationTaskInput, AiImageAssetTaskInput,
    AiProductAttributesTaskInput, AiProductCopyTaskInput, AiProviderConfig, ChatMessage,
    ChatMessageRole, DirectExecutionTarget, ProviderChatRequest, ProviderImageRequest,
    ProviderStreamEmitter, ToolTrace,
};
use crate::service::{AiHostRuntime, AiOperatorContext};
use crate::{AiError, AiResult};
use rustok_core::{CONTENT_FORMAT_MARKDOWN, SecurityContext};
#[path = "direct_content_moderation.rs"]
mod direct_content_moderation;
#[path = "direct_domain_alloy.rs"]
mod direct_domain_alloy;
#[path = "direct_domain_commerce.rs"]
mod direct_domain_commerce;
#[path = "direct_domain_content.rs"]
mod direct_domain_content;
#[path = "direct_domain_media.rs"]
mod direct_domain_media;
#[path = "direct_domain_orders.rs"]
mod direct_domain_orders;
#[path = "direct_order_generation.rs"]
mod direct_order_generation;
#[path = "direct_order_tasks.rs"]
mod direct_order_tasks;
#[path = "direct_product_attributes.rs"]
mod direct_product_attributes;
use direct_domain_alloy::register_alloy_direct_handlers;
use direct_domain_commerce::register_commerce_direct_handlers;
use direct_domain_content::register_content_direct_handlers;
use direct_domain_media::register_media_direct_handlers;
use direct_domain_orders::register_order_direct_handlers;
pub(crate) use direct_order_generation::{generate_order_analytics, generate_order_ops_assistant};

pub struct DirectExecutionRequest {
    pub task_slug: String,
    pub task_input_json: Value,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub system_prompt: Option<String>,
    pub provider_config: AiProviderConfig,
    pub provider: Arc<dyn InferenceEngine>,
    pub stream_emitter: Option<ProviderStreamEmitter>,
}

pub struct DirectExecutionResult {
    pub execution_target: DirectExecutionTarget,
    pub appended_messages: Vec<ChatMessage>,
    pub traces: Vec<ToolTrace>,
    pub metadata: Value,
}

pub(crate) fn direct_operator_port_context(
    operator: &AiOperatorContext,
    locale: &str,
    task_slug: &str,
    deadline: Duration,
) -> PortContext {
    operator.role_slugs.iter().fold(
        PortContext::new(
            operator.tenant_id.to_string(),
            PortActor::user(operator.user_id.to_string()),
            locale,
            format!("ai-direct:{task_slug}"),
        )
        .with_deadline(deadline),
        |context, role| context.with_role(role.clone()),
    )
}

#[async_trait]
pub trait DirectTaskHandler: Send + Sync {
    fn task_slug(&self) -> &'static str;

    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult>;
}

#[derive(Default)]
pub struct DirectExecutionRegistry {
    handlers: HashMap<&'static str, Arc<dyn DirectTaskHandler>>,
}

impl DirectExecutionRegistry {
    pub fn with_core_defaults() -> Self {
        let mut registry = Self::default();
        register_alloy_direct_handlers(&mut registry);
        register_media_direct_handlers(&mut registry);
        registry
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::with_core_defaults();
        register_content_direct_handlers(&mut registry);
        register_commerce_direct_handlers(&mut registry);
        register_order_direct_handlers(&mut registry);
        registry
    }

    pub fn register(&mut self, handler: Arc<dyn DirectTaskHandler>) {
        self.handlers.insert(handler.task_slug(), handler);
    }

    pub fn handler(&self, task_slug: &str) -> Option<Arc<dyn DirectTaskHandler>> {
        self.handlers.get(task_slug).map(Arc::clone)
    }
}

pub struct AlloyScriptAssistHandler;

#[async_trait]
impl DirectTaskHandler for AlloyScriptAssistHandler {
    fn task_slug(&self) -> &'static str {
        rustok_ai_alloy::ALLOY_CODE_TASK_SLUG
    }

    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiAlloyTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        rustok_ai_alloy::validate_runtime_payload(input.runtime_payload_json.as_deref())
            .map_err(AiError::Validation)?;
        let scoped = runtime
            .scoped_alloy_runtime(operator.tenant_id)
            .ok_or_else(|| AiError::Runtime("Alloy runtime is not initialized".to_string()))?;
        let started = std::time::Instant::now();

        let (trace_name, operation_payload, summary) = match input.operation {
            AlloyOperation::ListScripts => {
                let page = scoped
                    .storage
                    .find_paginated(alloy::ScriptQuery::All, 0, 100)
                    .await
                    .map_err(|err| AiError::Runtime(err.to_string()))?;
                let scripts = page
                    .items
                    .into_iter()
                    .map(|script| {
                        json!({
                            "id": script.id,
                            "name": script.name,
                            "status": script.status.as_str(),
                            "description": script.description,
                            "updated_at": script.updated_at.to_rfc3339(),
                        })
                    })
                    .collect::<Vec<_>>();
                (
                    "direct.alloy.list_scripts".to_string(),
                    json!({
                        "operation": input.operation.slug(),
                        "scripts": scripts,
                        "total": page.total,
                    }),
                    format!("Listed {} Alloy scripts.", page.total),
                )
            }
            AlloyOperation::GetScript => {
                let script =
                    resolve_script(&scoped.storage, input.script_id, input.script_name).await?;
                (
                    "direct.alloy.get_script".to_string(),
                    json!({
                        "operation": input.operation.slug(),
                        "script": {
                            "id": script.id,
                            "name": script.name,
                            "description": script.description,
                            "status": script.status.as_str(),
                            "version": script.version,
                            "workspace": script.workspace,
                            "trigger": script.trigger,
                        }
                    }),
                    format!("Loaded Alloy script `{}`.", script.name),
                )
            }
            AlloyOperation::ValidateScript => {
                let script_source = input
                    .script_source
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        AiError::Validation(
                            "script_source is required for validate_script".to_string(),
                        )
                    })?;
                let validation = serde_json::to_value(alloy_validate_script(
                    &AlloyMcpState::new(
                        scoped.storage.clone(),
                        scoped.engine.clone(),
                        scoped.orchestrator.clone(),
                    ),
                    ValidateScriptRequest {
                        code: script_source.clone(),
                    },
                ))
                .map_err(AiError::Json)?;
                let summary = if validation["valid"].as_bool().unwrap_or(false) {
                    "Validated Alloy script successfully.".to_string()
                } else {
                    format!(
                        "Alloy script validation failed: {}",
                        validation["message"].as_str().unwrap_or("unknown error")
                    )
                };
                (
                    "direct.alloy.validate_script".to_string(),
                    json!({
                        "operation": input.operation.slug(),
                        "validation": validation,
                    }),
                    summary,
                )
            }
            AlloyOperation::RunScript => {
                let script =
                    resolve_script(&scoped.storage, input.script_id, input.script_name).await?;
                let params = parse_runtime_payload(input.runtime_payload_json)?;
                let result = scoped
                    .orchestrator
                    .run_manual_with_entity(
                        &script.name,
                        params
                            .into_iter()
                            .map(|(key, value)| (key, json_to_dynamic(value)))
                            .collect(),
                        None,
                        None,
                    )
                    .await
                    .map_err(|err| AiError::Runtime(err.to_string()))?;
                let _ = scoped
                    .execution_log
                    .record_with_context(
                        &result,
                        Some(operator.user_id.to_string()),
                        Some(operator.tenant_id),
                    )
                    .await;
                let duration_ms = result.duration_ms();
                let execution_id = result.execution_id;

                let operation_payload = match result.outcome {
                    alloy::ExecutionOutcome::Success {
                        return_value,
                        entity_changes,
                    } => json!({
                        "operation": input.operation.slug(),
                        "script_id": script.id,
                        "script_name": script.name,
                        "success": true,
                        "execution_id": execution_id,
                        "duration_ms": duration_ms,
                        "return_value": return_value.map(dynamic_to_json),
                        "changes": entity_changes
                            .into_iter()
                            .map(|(key, value)| (key, dynamic_to_json(value)))
                            .collect::<serde_json::Map<String, Value>>(),
                    }),
                    alloy::ExecutionOutcome::Aborted { reason } => json!({
                        "operation": input.operation.slug(),
                        "script_id": script.id,
                        "script_name": script.name,
                        "success": false,
                        "execution_id": execution_id,
                        "duration_ms": duration_ms,
                        "error": reason,
                    }),
                    alloy::ExecutionOutcome::Failed { error } => json!({
                        "operation": input.operation.slug(),
                        "script_id": script.id,
                        "script_name": script.name,
                        "success": false,
                        "execution_id": execution_id,
                        "duration_ms": duration_ms,
                        "error": error.to_string(),
                    }),
                };
                let summary = if operation_payload["success"].as_bool().unwrap_or(false) {
                    format!("Executed Alloy script `{}` successfully.", script.name)
                } else {
                    format!(
                        "Alloy script `{}` failed: {}",
                        script.name,
                        operation_payload["error"]
                            .as_str()
                            .unwrap_or("execution failed")
                    )
                };
                (
                    "direct.alloy.run_script".to_string(),
                    operation_payload,
                    summary,
                )
            }
        };

        let trace = ToolTrace {
            tool_name: trace_name,
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: false,
            error_message: None,
            created_at: Utc::now(),
        };

        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;

        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Alloy,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "operation": input.operation.slug(),
                "operation_payload": operation_payload,
            }),
        })
    }
}

pub struct MediaImageAssetHandler;

pub struct ProductCopyHandler;

pub struct BlogDraftHandler;

#[async_trait]
impl DirectTaskHandler for MediaImageAssetHandler {
    fn task_slug(&self) -> &'static str {
        rustok_ai_media::IMAGE_ASSET_TASK_SLUG
    }

    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiImageAssetTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let provider_request = build_image_provider_request(
            &input,
            &request.provider_config.model,
            &request.resolved_locale,
        )?;
        let prompt = provider_request.prompt.clone();
        let requested_image_size = provider_request
            .size
            .clone()
            .unwrap_or_else(|| "1024x1024".to_string());

        let started = std::time::Instant::now();
        let provider_image = request
            .provider
            .generate_image(&request.provider_config, provider_request)
            .await?;

        let file_name = build_generated_file_name(
            input.file_name.as_deref(),
            input.title.as_deref(),
            &provider_image.mime_type,
        );
        let media_service = MediaService::new(runtime.db_clone(), storage_from_runtime(runtime)?);
        let media_item = media_service
            .upload(UploadInput {
                tenant_id: operator.tenant_id,
                uploaded_by: Some(operator.user_id),
                original_name: file_name.clone(),
                content_type: provider_image.mime_type.clone(),
                data: Bytes::from(provider_image.bytes),
            })
            .await
            .map_err(|err| AiError::Runtime(err.to_string()))?;

        let translation = media_service
            .upsert_translation(
                operator.tenant_id,
                media_item.id,
                UpsertTranslationInput {
                    locale: request.resolved_locale.clone(),
                    title: normalize_optional_text(input.title)
                        .or_else(|| Some(default_image_title(&request.resolved_locale))),
                    alt_text: normalize_optional_text(input.alt_text)
                        .or_else(|| Some(prompt.clone())),
                    caption: normalize_optional_text(input.caption),
                },
            )
            .await
            .map_err(|err| AiError::Runtime(err.to_string()))?;

        let operation_payload = json!({
            "media_item": {
                "id": media_item.id,
                "filename": media_item.filename,
                "original_name": media_item.original_name,
                "mime_type": media_item.mime_type,
                "public_url": media_item.public_url,
                "size": media_item.size,
                "width": media_item.width,
                "height": media_item.height,
                "metadata": media_item.metadata,
            },
            "translation": {
                "id": translation.id,
                "locale": translation.locale,
                "title": translation.title,
                "alt_text": translation.alt_text,
                "caption": translation.caption,
            },
            "image_generation": {
                "provider_slug": request.provider_config.provider_slug.as_str(),
                "model": request.provider_config.model,
                "size": requested_image_size,
                "revised_prompt": provider_image.revised_prompt,
            }
        });
        let summary = format!(
            "Generated media asset `{}` and stored it in the media library.",
            media_item.original_name
        );
        let trace = ToolTrace {
            tool_name: "direct.media.generate_image".to_string(),
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: false,
            error_message: None,
            created_at: Utc::now(),
        };
        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;

        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Media,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "media_item": {
                    "id": media_item.id,
                    "public_url": media_item.public_url,
                    "mime_type": media_item.mime_type,
                },
                "translation": {
                    "locale": translation.locale,
                },
            }),
        })
    }
}

#[async_trait]
impl DirectTaskHandler for ProductCopyHandler {
    fn task_slug(&self) -> &'static str {
        PRODUCT_COPY_TASK_SLUG
    }

    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiProductCopyTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let catalog = CatalogService::new(runtime.db_clone(), runtime.event_bus());
        let product = catalog
            .get_product(operator.tenant_id, input.product_id)
            .await
            .map_err(|err| AiError::Runtime(err.to_string()))?;

        let source_locale = normalize_locale_hint(input.source_locale.as_deref());
        let target_locale = request.resolved_locale.clone();
        let source_translation = resolve_product_source_translation(
            &product,
            source_locale.as_deref(),
            &target_locale,
            &input,
        )?;
        let current_target_translation = product
            .translations
            .iter()
            .find(|translation| locale_matches(&translation.locale, &target_locale));

        let generated_copy = generate_product_copy(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            &target_locale,
            &product,
            &source_translation,
            current_target_translation,
            input.copy_instructions.as_deref(),
        )
        .await?;

        let title = normalize_optional_text(generated_copy.title)
            .or_else(|| source_translation.title.clone())
            .ok_or_else(|| {
                AiError::Validation("product copy generation returned empty title".to_string())
            })?;
        let description = normalize_optional_text(generated_copy.description)
            .or(source_translation.description.clone());
        let meta_title = normalize_optional_text(generated_copy.meta_title)
            .or_else(|| {
                current_target_translation.and_then(|translation| translation.meta_title.clone())
            })
            .or_else(|| Some(title.clone()));
        let meta_description = normalize_optional_text(generated_copy.meta_description)
            .or_else(|| {
                current_target_translation
                    .and_then(|translation| translation.meta_description.clone())
            })
            .or_else(|| description.clone());
        let target_handle = current_target_translation
            .map(|translation| translation.handle.clone())
            .or_else(|| normalize_optional_text(generated_copy.handle));

        let mut translations = product
            .translations
            .iter()
            .filter(|translation| !locale_matches(&translation.locale, &target_locale))
            .map(|translation| ProductTranslationInput {
                locale: translation.locale.clone(),
                title: translation.title.clone(),
                handle: Some(translation.handle.clone()),
                description: translation.description.clone(),
                meta_title: translation.meta_title.clone(),
                meta_description: translation.meta_description.clone(),
            })
            .collect::<Vec<_>>();
        translations.push(ProductTranslationInput {
            locale: target_locale.clone(),
            title: title.clone(),
            handle: target_handle.clone(),
            description: description.clone(),
            meta_title: meta_title.clone(),
            meta_description: meta_description.clone(),
        });

        let updated = catalog
            .update_product(
                operator.tenant_id,
                operator.user_id,
                product.id,
                UpdateProductInput {
                    translations: Some(translations),
                    seller_id: None,
                    vendor: None,
                    product_type: None,
                    shipping_profile_slug: None,
                    primary_category_id: None,
                    tags: None,
                    metadata: None,
                    status: None,
                },
            )
            .await
            .map_err(|err| AiError::Runtime(err.to_string()))?;

        let target_translation = updated
            .translations
            .iter()
            .find(|translation| locale_matches(&translation.locale, &target_locale))
            .ok_or_else(|| {
                AiError::Runtime(format!(
                    "updated product is missing translation for locale `{target_locale}`"
                ))
            })?;

        let operation_payload = json!({
            "product": {
                "id": updated.id,
                "status": format!("{:?}", updated.status).to_lowercase(),
                "vendor": updated.vendor,
                "product_type": updated.product_type,
                "shipping_profile_slug": updated.shipping_profile_slug,
                "tags": updated.tags,
            },
            "source_translation": {
                "locale": source_translation.locale.clone(),
                "title": source_translation.title.clone(),
                "description": source_translation.description.clone(),
                "meta_title": source_translation.meta_title.clone(),
                "meta_description": source_translation.meta_description.clone(),
            },
            "target_translation": {
                "locale": target_translation.locale,
                "title": target_translation.title,
                "handle": target_translation.handle,
                "description": target_translation.description,
                "meta_title": target_translation.meta_title,
                "meta_description": target_translation.meta_description,
            }
        });
        let summary = format!(
            "Updated product `{}` copy for locale `{}`.",
            updated.id, target_locale
        );
        let trace = ToolTrace {
            tool_name: PRODUCT_COPY_TOOL_NAME.to_string(),
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: false,
            error_message: None,
            created_at: Utc::now(),
        };
        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;

        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Commerce,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "product_id": updated.id,
                "target_locale": target_locale,
                "source_locale": source_translation.locale.clone(),
            }),
        })
    }
}

#[async_trait]
impl DirectTaskHandler for BlogDraftHandler {
    fn task_slug(&self) -> &'static str {
        BLOG_DRAFT_TASK_SLUG
    }

    async fn execute(
        &self,
        runtime: &AiHostRuntime,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiBlogDraftTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let service = PostService::new(runtime.db_clone(), runtime.event_bus());
        let security = ai_security_context(operator);
        let source_locale = normalize_locale_hint(input.source_locale.as_deref());
        let source_lookup_locale = source_locale
            .clone()
            .unwrap_or_else(|| request.resolved_locale.clone());
        let existing_post = match input.post_id {
            Some(post_id) => Some(
                service
                    .get_post_with_locale_fallback(
                        operator.tenant_id,
                        security.clone(),
                        post_id,
                        &source_lookup_locale,
                        Some("en"),
                    )
                    .await
                    .map_err(|err| AiError::Runtime(err.to_string()))?,
            ),
            None => None,
        };
        let source =
            resolve_blog_source_content(existing_post.as_ref(), &input, &request.resolved_locale)?;
        let generated = generate_blog_draft(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            &request.resolved_locale,
            existing_post.as_ref(),
            &source,
            input.copy_instructions.as_deref(),
        )
        .await?;

        let title = normalize_optional_text(generated.title)
            .or_else(|| source.title.clone())
            .ok_or_else(|| {
                AiError::Validation("blog_draft generation returned empty title".to_string())
            })?;
        let body = normalize_optional_text(generated.body)
            .or_else(|| source.body.clone())
            .ok_or_else(|| {
                AiError::Validation("blog_draft generation returned empty body".to_string())
            })?;
        let excerpt = normalize_optional_text(generated.excerpt).or(source.excerpt.clone());
        let seo_title = normalize_optional_text(generated.seo_title)
            .or_else(|| source.seo_title.clone())
            .or_else(|| Some(title.clone()));
        let seo_description = normalize_optional_text(generated.seo_description)
            .or_else(|| source.seo_description.clone())
            .or_else(|| excerpt.clone());
        let slug = normalize_optional_text(generated.slug);
        let tags = normalize_tag_list(&input.tags);

        let post_id = if let Some(existing_post) = existing_post.as_ref() {
            service
                .update_post(
                    operator.tenant_id,
                    existing_post.id,
                    security.clone(),
                    UpdatePostInput {
                        locale: Some(request.resolved_locale.clone()),
                        title: Some(title.clone()),
                        body: Some(body.clone()),
                        body_format: Some(CONTENT_FORMAT_MARKDOWN.to_string()),
                        content_json: None,
                        content: None,
                        excerpt: excerpt.clone(),
                        slug: slug.clone(),
                        tags: if tags.is_empty() {
                            None
                        } else {
                            Some(tags.clone())
                        },
                        category_id: input.category_id,
                        featured_image_url: input.featured_image_url.clone(),
                        seo_title: seo_title.clone(),
                        seo_description: seo_description.clone(),
                        channel_slugs: None,
                        metadata: None,
                        version: Some(existing_post.version),
                    },
                )
                .await
                .map_err(|err| AiError::Runtime(err.to_string()))?;
            existing_post.id
        } else {
            service
                .create_post(
                    operator.tenant_id,
                    security.clone(),
                    build_blog_draft_create_input(
                        &input,
                        &request.resolved_locale,
                        &title,
                        &body,
                        excerpt.as_deref(),
                        slug.as_deref(),
                        &tags,
                        seo_title.as_deref(),
                        seo_description.as_deref(),
                    )?,
                )
                .await
                .map_err(|err| AiError::Runtime(err.to_string()))?
        };

        let saved_post = service
            .get_post_with_locale_fallback(
                operator.tenant_id,
                security,
                post_id,
                &request.resolved_locale,
                Some("en"),
            )
            .await
            .map_err(|err| AiError::Runtime(err.to_string()))?;

        let operation_payload = json!({
            "post": {
                "id": saved_post.id,
                "title": saved_post.title,
                "slug": saved_post.slug,
                "locale": saved_post.locale,
                "effective_locale": saved_post.effective_locale,
                "status": format!("{:?}", saved_post.status).to_lowercase(),
                "excerpt": saved_post.excerpt,
                "seo_title": saved_post.seo_title,
                "seo_description": saved_post.seo_description,
                "tags": saved_post.tags,
                "category_id": saved_post.category_id,
                "featured_image_url": saved_post.featured_image_url,
                "version": saved_post.version,
            },
            "source": {
                "locale": source.locale.clone(),
                "title": source.title.clone(),
                "body": source.body.clone(),
                "excerpt": source.excerpt.clone(),
                "seo_title": source.seo_title.clone(),
                "seo_description": source.seo_description.clone(),
            },
            "operation": if input.post_id.is_some() { "update_translation" } else { "create_draft" },
        });
        let summary = if input.post_id.is_some() {
            format!(
                "Updated blog post `{}` draft copy for locale `{}`.",
                saved_post.id, request.resolved_locale
            )
        } else {
            format!(
                "Created blog draft `{}` in locale `{}`.",
                saved_post.id, request.resolved_locale
            )
        };
        let trace = ToolTrace {
            tool_name: BLOG_DRAFT_TOOL_NAME.to_string(),
            input_payload: request.task_input_json.clone(),
            output_payload: Some(operation_payload.clone()),
            status: "completed".to_string(),
            duration_ms: started.elapsed().as_millis() as i64,
            sensitive: false,
            error_message: None,
            created_at: Utc::now(),
        };
        let explanation = explain_result(
            &request.provider,
            &request.provider_config,
            request.system_prompt.as_deref(),
            request.resolved_locale.as_str(),
            input.assistant_prompt.as_deref(),
            &summary,
            &operation_payload,
            request.stream_emitter.clone(),
        )
        .await;

        Ok(DirectExecutionResult {
            execution_target: DirectExecutionTarget::Blog,
            appended_messages: vec![explanation],
            traces: vec![trace],
            metadata: json!({
                "direct_task": request.task_slug,
                "requested_locale": request.requested_locale,
                "resolved_locale": request.resolved_locale,
                "post_id": saved_post.id,
                "operation": if input.post_id.is_some() { "update_translation" } else { "create_draft" },
            }),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct ProductSourceTranslation {
    locale: String,
    title: Option<String>,
    description: Option<String>,
    meta_title: Option<String>,
    meta_description: Option<String>,
}

fn resolve_product_source_translation(
    product: &rustok_product::dto::ProductResponse,
    source_locale: Option<&str>,
    target_locale: &str,
    input: &AiProductCopyTaskInput,
) -> AiResult<ProductSourceTranslation> {
    let selected = source_locale
        .and_then(|locale| {
            product
                .translations
                .iter()
                .find(|translation| locale_matches(&translation.locale, locale))
        })
        .or_else(|| {
            product
                .translations
                .iter()
                .find(|translation| !locale_matches(&translation.locale, target_locale))
        })
        .or_else(|| {
            product
                .translations
                .iter()
                .find(|translation| locale_matches(&translation.locale, target_locale))
        });

    let fallback_locale = source_locale
        .map(ToString::to_string)
        .or_else(|| selected.map(|translation| translation.locale.clone()))
        .unwrap_or_else(|| "en".to_string());

    let candidate = ProductSourceTranslation {
        locale: fallback_locale,
        title: normalize_optional_text(input.source_title.clone())
            .or_else(|| selected.map(|translation| translation.title.clone())),
        description: normalize_optional_text(input.source_description.clone())
            .or_else(|| selected.and_then(|translation| translation.description.clone())),
        meta_title: normalize_optional_text(input.source_meta_title.clone())
            .or_else(|| selected.and_then(|translation| translation.meta_title.clone())),
        meta_description: normalize_optional_text(input.source_meta_description.clone())
            .or_else(|| selected.and_then(|translation| translation.meta_description.clone())),
    };

    if candidate.title.is_none()
        && candidate.description.is_none()
        && candidate.meta_title.is_none()
        && candidate.meta_description.is_none()
    {
        return Err(AiError::Validation(
            "product_copy requires an existing source translation or source_* overrides"
                .to_string(),
        ));
    }

    Ok(candidate)
}

#[derive(Debug, Clone, Serialize)]
struct BlogSourceContent {
    locale: String,
    title: Option<String>,
    body: Option<String>,
    excerpt: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
}

fn resolve_blog_source_content(
    existing_post: Option<&rustok_blog::PostResponse>,
    input: &AiBlogDraftTaskInput,
    target_locale: &str,
) -> AiResult<BlogSourceContent> {
    let locale = existing_post
        .map(|post| post.locale.clone())
        .or_else(|| normalize_locale_hint(input.source_locale.as_deref()))
        .unwrap_or_else(|| target_locale.to_string());
    let candidate = BlogSourceContent {
        locale,
        title: normalize_optional_text(input.source_title.clone())
            .or_else(|| existing_post.map(|post| post.title.clone())),
        body: normalize_optional_text(input.source_body.clone())
            .or_else(|| existing_post.map(|post| post.body.clone())),
        excerpt: normalize_optional_text(input.source_excerpt.clone())
            .or_else(|| existing_post.and_then(|post| post.excerpt.clone())),
        seo_title: normalize_optional_text(input.source_seo_title.clone())
            .or_else(|| existing_post.and_then(|post| post.seo_title.clone())),
        seo_description: normalize_optional_text(input.source_seo_description.clone())
            .or_else(|| existing_post.and_then(|post| post.seo_description.clone())),
    };

    if candidate.title.is_none()
        && candidate.body.is_none()
        && candidate.excerpt.is_none()
        && candidate.seo_title.is_none()
        && candidate.seo_description.is_none()
        && normalize_optional_text(input.copy_instructions.clone()).is_none()
    {
        return Err(AiError::Validation(
            "blog_draft requires an existing post, source_* overrides, or copy_instructions"
                .to_string(),
        ));
    }

    Ok(candidate)
}

async fn generate_blog_draft(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    existing_post: Option<&rustok_blog::PostResponse>,
    source: &BlogSourceContent,
    copy_instructions: Option<&str>,
) -> AiResult<GeneratedBlogDraft> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `title`, `slug`, `body`, `excerpt`, `seo_title`, ",
        "`seo_description`. Write all text values in the target locale. `slug` may be null."
    );
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction.to_string(),
    };
    let prompt = json!({
        "task": "blog_draft",
        "target_locale": target_locale,
        "existing_post": existing_post.map(|post| json!({
            "id": post.id,
            "slug": post.slug,
            "status": format!("{:?}", post.status).to_lowercase(),
            "tags": post.tags,
            "category_id": post.category_id,
            "featured_image_url": post.featured_image_url,
        })),
        "source": source,
        "instructions": copy_instructions,
    })
    .to_string();

    let generated: GeneratedBlogDraft = complete_typed(
        provider,
        ProviderChatRequest {
            model: provider_config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: ChatMessageRole::System,
                    content: Some(system),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({
                        "locale": target_locale,
                        "direct_generation": "blog_draft",
                    }),
                },
                ChatMessage {
                    role: ChatMessageRole::User,
                    content: Some(prompt),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({
                        "locale": target_locale,
                        "direct_generation": "blog_draft",
                    }),
                },
            ],
            tools: Vec::new(),
            temperature: provider_config.temperature,
            max_tokens: provider_config.max_tokens,
            locale: Some(target_locale.to_string()),
        },
    )
    .await?;
    validate_blog_draft_payload(&generated).map_err(AiError::Validation)?;
    Ok(generated)
}

pub(crate) async fn generate_content_moderation(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiContentModerationTaskInput,
) -> AiResult<GeneratedModerationDecision> {
    let title = normalize_optional_text(input.title.clone());
    let body = normalize_optional_text(input.body.clone());
    if title.is_none() && body.is_none() {
        return Err(AiError::Validation(
            "content_moderation requires title or body".to_string(),
        ));
    }
    let locale_instruction = concat!(
        "Return valid JSON only with keys `decision`, `labels`, `severity`, `explanation`, ",
        "`requires_human`, `recommended_action`. ",
        "`decision` must be one of: allow, review, block. ",
        "`severity` must be an integer from 0 to 100."
    );
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction.to_string(),
    };
    let prompt = json!({
        "task": "content_moderation",
        "target_locale": target_locale,
        "content": {
            "id": input.content_id,
            "type": input.content_type,
            "locale": input.locale,
            "title": title,
            "body": body,
        }
    })
    .to_string();

    let decision: GeneratedModerationDecision = complete_typed(
        provider,
        ProviderChatRequest {
            model: provider_config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: ChatMessageRole::System,
                    content: Some(system),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({
                        "locale": target_locale,
                        "direct_generation": "content_moderation",
                    }),
                },
                ChatMessage {
                    role: ChatMessageRole::User,
                    content: Some(prompt),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({
                        "locale": target_locale,
                        "direct_generation": "content_moderation",
                    }),
                },
            ],
            tools: Vec::new(),
            temperature: provider_config.temperature,
            max_tokens: provider_config.max_tokens,
            locale: Some(target_locale.to_string()),
        },
    )
    .await?;

    validate_moderation_decision(&decision).map_err(AiError::Validation)
}

pub(crate) async fn generate_product_attributes(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    input: &AiProductAttributesTaskInput,
    product: Option<&rustok_product::dto::ProductResponse>,
) -> AiResult<GeneratedProductAttributes> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys: `brand`, `material`, `color`, `size`, `dimensions`, ",
        "`compatibility`, `care_instructions`, `hazmat`, `flex_attributes`. ",
        "`flex_attributes` must be an array of `{key, value}` objects with non-empty strings."
    );
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction.to_string(),
    };
    let prompt = json!({
        "task": "product_attributes",
        "target_locale": target_locale,
        "product": {
            "id": input.product_id,
            "catalog_projection": product.map(|product| json!({
                "id": product.id,
                "product_type": product.product_type,
                "vendor": product.vendor,
            })),
            "category_slug": input.category_slug,
            "source_title": input.source_title,
            "source_description": input.source_description,
            "image_urls": input.image_urls,
            "instructions": input.copy_instructions,
        }
    })
    .to_string();
    let generated: GeneratedProductAttributes = complete_typed(
        provider,
        ProviderChatRequest {
                model: provider_config.model.clone(),
                messages: vec![
                    ChatMessage {
                        role: ChatMessageRole::System,
                        content: Some(system),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": "product_attributes"}),
                    },
                    ChatMessage {
                        role: ChatMessageRole::User,
                        content: Some(prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({"locale": target_locale, "direct_generation": "product_attributes"}),
                    },
                ],
                tools: Vec::new(),
                temperature: provider_config.temperature,
                max_tokens: provider_config.max_tokens,
                locale: Some(target_locale.to_string()),
        },
    )
    .await?;
    validate_product_attributes_payload(&generated).map_err(AiError::Validation)?;
    Ok(generated)
}

#[allow(clippy::too_many_arguments)]
async fn generate_product_copy(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    product: &rustok_product::dto::ProductResponse,
    source_translation: &ProductSourceTranslation,
    current_target_translation: Option<&rustok_product::dto::ProductTranslationResponse>,
    copy_instructions: Option<&str>,
) -> AiResult<GeneratedProductCopy> {
    let locale_instruction = concat!(
        "Return valid JSON only with keys `title`, `handle`, `description`, `meta_title`, ",
        "`meta_description`. Write all text values in the target locale. `handle` may be null."
    );
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction.to_string(),
    };
    let prompt = json!({
        "task": "product_copy",
        "target_locale": target_locale,
        "product": {
            "id": product.id,
            "vendor": product.vendor,
            "product_type": product.product_type,
            "shipping_profile_slug": product.shipping_profile_slug,
            "tags": product.tags,
        },
        "source_translation": source_translation,
        "current_target_translation": current_target_translation,
        "instructions": copy_instructions,
    })
    .to_string();

    let generated: GeneratedProductCopy = complete_typed(
        provider,
        ProviderChatRequest {
            model: provider_config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: ChatMessageRole::System,
                    content: Some(system),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({
                        "locale": target_locale,
                        "direct_generation": "product_copy",
                    }),
                },
                ChatMessage {
                    role: ChatMessageRole::User,
                    content: Some(prompt),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({
                        "locale": target_locale,
                        "direct_generation": "product_copy",
                    }),
                },
            ],
            tools: Vec::new(),
            temperature: provider_config.temperature,
            max_tokens: provider_config.max_tokens,
            locale: Some(target_locale.to_string()),
        },
    )
    .await?;
    validate_product_copy_payload(&generated).map_err(AiError::Validation)?;
    Ok(generated)
}

pub(crate) async fn complete_typed<T>(
    provider: &Arc<dyn InferenceEngine>,
    request: ProviderChatRequest,
) -> AiResult<T>
where
    T: for<'de> serde::Deserialize<'de> + schemars::JsonSchema,
{
    let schema = serde_json::to_value(schemars::schema_for!(T)).map_err(AiError::Json)?;
    let value = provider
        .complete_structured(crate::model::ProviderStructuredRequest {
            request,
            output_schema: schema,
        })
        .await?;
    serde_json::from_value(value).map_err(AiError::Json)
}

fn normalize_locale_hint(locale: Option<&str>) -> Option<String> {
    locale.and_then(|value| {
        let normalized = value.trim().replace('_', "-");
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn locale_matches(left: &str, right: &str) -> bool {
    normalize_locale_hint(Some(left))
        .zip(normalize_locale_hint(Some(right)))
        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(&right))
}

pub(crate) fn ai_security_context(operator: &AiOperatorContext) -> SecurityContext {
    SecurityContext::from_permissions(
        infer_user_role_from_permissions(&operator.permissions),
        Some(operator.user_id),
        operator.permissions.iter().copied(),
    )
}

fn normalize_tag_list(tags: &[String]) -> Vec<String> {
    tags.iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_blog_draft_create_input(
    task_input: &AiBlogDraftTaskInput,
    locale: &str,
    title: &str,
    body: &str,
    excerpt: Option<&str>,
    slug: Option<&str>,
    tags: &[String],
    seo_title: Option<&str>,
    seo_description: Option<&str>,
) -> AiResult<CreatePostInput> {
    if !blog_draft_must_remain_unpublished() {
        return Err(AiError::InvalidConfig(
            "blog_draft policy must require draft review before persistence".to_string(),
        ));
    }

    Ok(CreatePostInput {
        locale: locale.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        body_format: CONTENT_FORMAT_MARKDOWN.to_string(),
        content_json: None,
        content: None,
        excerpt: excerpt.map(ToString::to_string),
        slug: slug.map(ToString::to_string),
        publish: false,
        tags: tags.to_vec(),
        category_id: task_input.category_id,
        featured_image_url: task_input.featured_image_url.clone(),
        seo_title: seo_title.map(ToString::to_string),
        seo_description: seo_description.map(ToString::to_string),
        channel_slugs: None,
        metadata: None,
    })
}

fn build_image_provider_request(
    task_input: &AiImageAssetTaskInput,
    model: &str,
    resolved_locale: &str,
) -> AiResult<ProviderImageRequest> {
    let prompt = task_input.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AiError::Validation(
            "prompt is required for image_asset".to_string(),
        ));
    }

    Ok(ProviderImageRequest {
        model: model.to_string(),
        prompt,
        negative_prompt: task_input.negative_prompt.clone(),
        size: rustok_ai_media::normalize_image_size(task_input.size.clone())
            .map_err(AiError::Validation)?,
        locale: Some(resolved_locale.to_string()),
    })
}

fn storage_from_runtime(runtime: &AiHostRuntime) -> AiResult<StorageRuntime> {
    runtime.storage().ok_or_else(|| {
        AiError::Runtime("StorageRuntime is not registered in AI runtime".to_string())
    })
}

fn build_generated_file_name(
    explicit_file_name: Option<&str>,
    title: Option<&str>,
    mime_type: &str,
) -> String {
    let extension = mime_extension(mime_type);
    if let Some(file_name) = explicit_file_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let base = sanitize_file_stem(file_name);
        if base.ends_with(&format!(".{extension}")) {
            return base;
        }
        return format!("{base}.{extension}");
    }

    let stem = title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_file_stem)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("ai-image-{}", Utc::now().format("%Y%m%d%H%M%S")));
    format!("{stem}.{extension}")
}

fn sanitize_file_stem(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('-').trim_matches('.').to_string()
}

fn mime_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn default_image_title(locale: &str) -> String {
    format!("[{locale}] AI image")
}

async fn resolve_script(
    registry: &Arc<alloy::SeaOrmStorage>,
    script_id: Option<Uuid>,
    script_name: Option<String>,
) -> AiResult<alloy::Script> {
    match (
        script_id,
        script_name.filter(|value| !value.trim().is_empty()),
    ) {
        (Some(id), _) => registry
            .get(id)
            .await
            .map_err(|err| AiError::Runtime(err.to_string())),
        (None, Some(name)) => registry
            .get_by_name(name.trim())
            .await
            .map_err(|err| AiError::Runtime(err.to_string())),
        (None, None) => Err(AiError::Validation(
            "script_id or script_name is required".to_string(),
        )),
    }
}

fn parse_runtime_payload(payload: Option<String>) -> AiResult<serde_json::Map<String, Value>> {
    let Some(payload) = payload.filter(|value| !value.trim().is_empty()) else {
        return Ok(serde_json::Map::new());
    };
    let parsed: Value = serde_json::from_str(&payload)?;
    let object = parsed.as_object().cloned().ok_or_else(|| {
        AiError::Validation("runtime_payload_json must be a JSON object".to_string())
    })?;
    Ok(object)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn explain_result(
    provider: &Arc<dyn InferenceEngine>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    locale: &str,
    assistant_prompt: Option<&str>,
    summary: &str,
    payload: &Value,
    stream_emitter: Option<ProviderStreamEmitter>,
) -> ChatMessage {
    let locale_instruction =
        format!("Respond in locale `{locale}`. Keep the answer concise and operator-facing.");
    let system = match system_prompt {
        Some(system_prompt) if !system_prompt.trim().is_empty() => {
            format!("{system_prompt}\n\n{locale_instruction}")
        }
        _ => locale_instruction,
    };
    let prompt = json!({
        "assistant_prompt": assistant_prompt,
        "summary": summary,
        "result": payload,
    })
    .to_string();

    match provider
        .complete_stream(
            provider_config,
            ProviderChatRequest {
                model: provider_config.model.clone(),
                messages: vec![
                    ChatMessage {
                        role: ChatMessageRole::System,
                        content: Some(system),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({ "locale": locale, "direct_explanation": true }),
                    },
                    ChatMessage {
                        role: ChatMessageRole::User,
                        content: Some(prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                        metadata: json!({ "locale": locale, "direct_explanation": true }),
                    },
                ],
                tools: Vec::new(),
                temperature: provider_config.temperature,
                max_tokens: provider_config.max_tokens,
                locale: Some(locale.to_string()),
            },
            stream_emitter.clone(),
        )
        .await
    {
        Ok(response) => ChatMessage {
            metadata: merge_message_metadata(
                response.assistant_message.metadata,
                json!({
                    "locale": locale,
                    "direct_explanation": true,
                }),
            ),
            ..response.assistant_message
        },
        Err(error) => ChatMessage {
            role: ChatMessageRole::Assistant,
            content: {
                let content = format!("[{locale}] {summary}");
                if let Some(emitter) = stream_emitter {
                    emitter.emit_text_delta(content.clone());
                }
                Some(content)
            },
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
            metadata: json!({
                "locale": locale,
                "direct_explanation": true,
                "provider_error": error.to_string(),
            }),
        },
    }
}

fn merge_message_metadata(base: Value, extension: Value) -> Value {
    if !base.is_object() && !extension.is_object() {
        return json!({});
    }

    let mut merged = match base {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    if let Value::Object(extension) = extension {
        for (key, value) in extension {
            merged.insert(key, value);
        }
    }
    Value::Object(merged)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use super::direct_order_tasks::{OrderAnalyticsHandler, OrderOpsAssistantHandler};
    use super::direct_product_attributes::ProductAttributesHandler;
    use super::{
        BlogDraftHandler, DirectExecutionRequest, DirectTaskHandler, MediaImageAssetHandler,
        ProductCopyHandler, build_blog_draft_create_input, build_generated_file_name,
        build_image_provider_request, locale_matches, normalize_tag_list,
    };
    use crate::{
        AiHostRuntime, AiOperatorContext, AiProviderTargetCatalog, ProviderEgressPolicy,
        ProviderSlug, ProviderTargetAuth,
        engine::InferenceEngine,
        model::{
            AiBlogDraftTaskInput, AiImageAssetTaskInput, AiOrderAnalyticsTaskInput,
            AiOrderOpsAssistantTaskInput, AiProductAttributesTaskInput, AiProductCopyTaskInput,
            AiProviderConfig, ChatMessage, ChatMessageRole, DirectExecutionTarget,
            ProviderChatRequest, ProviderChatResponse, ProviderImageRequest, ProviderImageResponse,
            ProviderStructuredRequest, ProviderTestResult,
        },
    };
    use async_trait::async_trait;
    use rust_decimal::Decimal;
    use rustok_ai_content::{BLOG_DRAFT_TASK_SLUG, content_ai_verticals};
    use rustok_ai_media::{media_ai_verticals, normalize_image_size};
    use rustok_ai_order::order_ai_verticals;
    use rustok_ai_product::product_ai_verticals;
    use rustok_api::{PortContext, PortError};
    use rustok_core::{Rbac, UserRole, registry::ModuleRegistry};
    use rustok_order::{
        CheckoutCompletionPort, CheckoutCompletionSnapshot, CheckoutResultRequest,
        CompleteCheckoutPortRequest, OrderStatusRequest, OrderStatusSnapshot,
    };
    use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
    use rustok_product::{CatalogService, ProductCatalogReadPort, ProductProjectionRequest};
    use rustok_secrets::SecretResolverRegistry;
    use rustok_storage::{LocalStorageConfig, StorageConfig, StorageDriver, StorageRuntime};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use serde_json::json;
    use uuid::Uuid;

    struct BlogDraftEngine;

    struct ImageAssetEngine {
        captured_request: Arc<Mutex<Option<ProviderImageRequest>>>,
    }

    struct ProductCopyEngine;

    struct ProductAttributesEngine;

    struct OrderAnalyticsEngine;

    struct OrderOpsAssistantEngine;

    struct RecordingOrderStatusPort {
        calls: Arc<Mutex<Vec<(PortContext, OrderStatusRequest)>>>,
        response: Result<OrderStatusSnapshot, PortError>,
        delay: Option<std::time::Duration>,
    }

    struct RecordingProductCatalogReadPort {
        calls: Arc<Mutex<Vec<(PortContext, ProductProjectionRequest)>>>,
        response: Result<rustok_product::dto::ProductResponse, PortError>,
        delay: Option<std::time::Duration>,
    }

    #[async_trait]
    impl ProductCatalogReadPort for RecordingProductCatalogReadPort {
        async fn read_product_projection(
            &self,
            context: PortContext,
            request: ProductProjectionRequest,
        ) -> Result<rustok_product::dto::ProductResponse, PortError> {
            self.calls
                .lock()
                .expect("product catalog read calls lock")
                .push((context, request));
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }
            self.response.clone()
        }

        async fn read_variant_product_projection(
            &self,
            _context: PortContext,
            _request: rustok_product::VariantProductProjectionRequest,
        ) -> Result<rustok_product::dto::ProductResponse, PortError> {
            unreachable!("AI product attributes use the product-id projection")
        }

        async fn list_published_products(
            &self,
            _context: PortContext,
            _request: rustok_product::PublishedProductsRequest,
        ) -> Result<rustok_product::StorefrontProductList, PortError> {
            unreachable!("AI product attributes do not list storefront products")
        }
    }

    #[async_trait]
    impl CheckoutCompletionPort for RecordingOrderStatusPort {
        async fn complete_checkout(
            &self,
            _context: PortContext,
            _request: CompleteCheckoutPortRequest,
        ) -> Result<CheckoutCompletionSnapshot, PortError> {
            unreachable!("AI order enrichment performs only read_order_status")
        }

        async fn read_checkout_result(
            &self,
            _context: PortContext,
            _request: CheckoutResultRequest,
        ) -> Result<CheckoutCompletionSnapshot, PortError> {
            unreachable!("AI order enrichment performs only read_order_status")
        }

        async fn read_checkout_result_by_operation(
            &self,
            _context: PortContext,
            _request: rustok_order::CheckoutResultByOperationRequest,
        ) -> Result<CheckoutCompletionSnapshot, PortError> {
            unreachable!("AI order enrichment performs only read_order_status")
        }

        async fn read_order_status(
            &self,
            context: PortContext,
            request: OrderStatusRequest,
        ) -> Result<OrderStatusSnapshot, PortError> {
            self.calls
                .lock()
                .expect("order status calls lock")
                .push((context, request));
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }
            self.response.clone()
        }
    }

    #[async_trait]
    impl InferenceEngine for BlogDraftEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct draft execution does not probe connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("blog draft uses typed generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Draft saved for editorial review.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            Ok(json!({
                "title": "Generated draft",
                "slug": "generated-draft",
                "body": "Generated draft body",
                "excerpt": "Generated excerpt",
                "seo_title": "Generated SEO title",
                "seo_description": "Generated SEO description"
            }))
        }
    }

    #[async_trait]
    impl InferenceEngine for ImageAssetEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct image execution does not probe connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("direct image execution uses streaming explanation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Image saved to the media library.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            unreachable!("direct image execution does not use typed text generation")
        }

        async fn generate_image(
            &self,
            _config: &AiProviderConfig,
            request: ProviderImageRequest,
        ) -> crate::AiResult<ProviderImageResponse> {
            *self
                .captured_request
                .lock()
                .expect("captured image provider request") = Some(request);
            Ok(ProviderImageResponse {
                bytes: b"\x89PNG\r\n\x1a\nimage".to_vec(),
                mime_type: "image/png".to_string(),
                revised_prompt: Some("Revised editorial hero image".to_string()),
                raw_payload: json!({}),
            })
        }
    }

    #[async_trait]
    impl InferenceEngine for ProductCopyEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct product copy execution does not probe connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("direct product copy execution uses typed generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Product copy saved for the selected locale.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            Ok(json!({
                "title": "Generated product",
                "handle": "generated-product",
                "description": "Generated description",
                "meta_title": "Generated SEO title",
                "meta_description": "Generated SEO description"
            }))
        }
    }

    #[async_trait]
    impl InferenceEngine for ProductAttributesEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct product attributes execution does not probe connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("direct product attributes execution uses typed generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Product attributes are ready for review.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            Ok(json!({
                "brand": "Example brand",
                "material": "Cotton",
                "color": "Blue",
                "size": null,
                "dimensions": null,
                "compatibility": null,
                "care_instructions": "Machine wash cold",
                "hazmat": null,
                "flex_attributes": [{"key": "fabric_weight", "value": "180 gsm"}]
            }))
        }
    }

    #[async_trait]
    impl InferenceEngine for OrderAnalyticsEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct order analytics execution does not probe connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("direct order analytics execution uses typed generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Order analytics are ready for review.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            Ok(json!({
                "summary": "One order needs an address review.",
                "key_findings": ["Address format differs from prior orders"],
                "risk_flags": ["address_mismatch"],
                "recommended_actions": ["Ask an operator to confirm the address"]
            }))
        }
    }

    #[async_trait]
    impl InferenceEngine for OrderOpsAssistantEngine {
        async fn test_connection(
            &self,
            _config: &AiProviderConfig,
        ) -> crate::AiResult<ProviderTestResult> {
            unreachable!("direct order operations execution does not probe connectivity")
        }

        async fn complete(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
        ) -> crate::AiResult<ProviderChatResponse> {
            unreachable!("direct order operations execution uses typed generation")
        }

        async fn complete_stream(
            &self,
            _config: &AiProviderConfig,
            _request: ProviderChatRequest,
            _emitter: Option<crate::ProviderStreamEmitter>,
        ) -> crate::AiResult<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                assistant_message: ChatMessage {
                    role: ChatMessageRole::Assistant,
                    content: Some("Order operation is ready for review.".to_string()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                    metadata: json!({}),
                },
                finish_reason: Some("stop".to_string()),
                raw_payload: json!({}),
            })
        }

        async fn complete_structured(
            &self,
            _request: ProviderStructuredRequest,
        ) -> crate::AiResult<serde_json::Value> {
            Ok(json!({
                "recommended_action": "contact_customer",
                "rationale": "The shipping address needs confirmation.",
                "prefill": {"message": "Please confirm your shipping address."},
                "requires_human": true,
                "confidence": 85
            }))
        }
    }

    fn provider_config() -> AiProviderConfig {
        AiProviderConfig {
            tenant_id: Uuid::nil(),
            provider_slug: ProviderSlug::new("openai_compatible").unwrap(),
            target_auth: ProviderTargetAuth::SecretRefs,
            model: "test-model".to_string(),
            settings: BTreeMap::new(),
            credential_refs: BTreeMap::new(),
            temperature: None,
            max_tokens: None,
            capabilities: Vec::new(),
            usage_policy: Default::default(),
        }
    }

    async fn blog_runtime() -> (AiHostRuntime, AiOperatorContext) {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("blog draft database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE taxonomy_terms (id TEXT PRIMARY KEY)".to_string(),
            ))
            .await
            .expect("taxonomy term fixture");
        let manager = SchemaManager::new(&database);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration");
        for migration in rustok_blog::migrations::migrations() {
            migration.up(&manager).await.expect("blog migration");
        }

        let runtime = AiHostRuntime::new(
            database.clone(),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database))),
            ModuleRegistry::new(),
            SecretResolverRegistry::builder().build(),
            ProviderEgressPolicy::default(),
            AiProviderTargetCatalog::default(),
        );
        (runtime, admin_operator())
    }

    async fn media_runtime(storage: StorageRuntime) -> (AiHostRuntime, AiOperatorContext) {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("media image database");
        for statement in [
            "CREATE TABLE media (\
                id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, uploaded_by TEXT NULL, \
                filename TEXT NOT NULL, original_name TEXT NOT NULL, mime_type TEXT NOT NULL, \
                size INTEGER NOT NULL, storage_path TEXT NOT NULL, storage_driver TEXT NOT NULL, \
                width INTEGER NULL, height INTEGER NULL, metadata TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE media_translations (\
                id TEXT PRIMARY KEY, media_id TEXT NOT NULL, locale TEXT NOT NULL, title TEXT NULL, \
                alt_text TEXT NULL, caption TEXT NULL, UNIQUE(media_id, locale)\
             )",
        ] {
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("media fixture schema");
        }
        let manager = SchemaManager::new(&database);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration");

        let runtime = AiHostRuntime::new(
            database.clone(),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database))),
            ModuleRegistry::new(),
            SecretResolverRegistry::builder().build(),
            ProviderEgressPolicy::default(),
            AiProviderTargetCatalog::default(),
        )
        .with_storage(Some(storage));
        (runtime, admin_operator())
    }

    async fn product_runtime() -> (AiHostRuntime, AiOperatorContext, Uuid) {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("product copy database");
        for statement in [
            "CREATE TABLE products (\
                id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, status TEXT NOT NULL, \
                seller_id TEXT NULL, vendor TEXT NULL, product_type TEXT NULL, \
                shipping_profile_slug TEXT NULL, primary_category_id TEXT NULL, metadata TEXT NOT NULL, \
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL, published_at TEXT NULL\
             )",
            "CREATE TABLE product_translations (\
                id TEXT PRIMARY KEY, product_id TEXT NOT NULL, tenant_id TEXT NOT NULL, locale TEXT NOT NULL, \
                title TEXT NOT NULL, handle TEXT NOT NULL, description TEXT NULL, meta_title TEXT NULL, \
                meta_description TEXT NULL\
             )",
            "CREATE TABLE product_options (id TEXT PRIMARY KEY, product_id TEXT NOT NULL, position INTEGER NOT NULL)",
            "CREATE TABLE product_variants (\
                id TEXT PRIMARY KEY, product_id TEXT NOT NULL, tenant_id TEXT NOT NULL, sku TEXT NULL, \
                barcode TEXT NULL, shipping_profile_slug TEXT NULL, ean TEXT NULL, upc TEXT NULL, \
                inventory_policy TEXT NOT NULL, inventory_management TEXT NOT NULL, inventory_quantity INTEGER NOT NULL, \
                weight TEXT NULL, weight_unit TEXT NULL, option1 TEXT NULL, option2 TEXT NULL, option3 TEXT NULL, \
                position INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE product_images (id TEXT PRIMARY KEY, product_id TEXT NOT NULL, media_id TEXT NOT NULL, position INTEGER NOT NULL, alt_text TEXT NULL)",
            "CREATE TABLE product_option_translations (id TEXT PRIMARY KEY, option_id TEXT NOT NULL, locale TEXT NOT NULL, title TEXT NOT NULL)",
            "CREATE TABLE product_option_values (id TEXT PRIMARY KEY, option_id TEXT NOT NULL, position INTEGER NOT NULL, metadata TEXT NOT NULL)",
            "CREATE TABLE product_option_value_translations (id TEXT PRIMARY KEY, value_id TEXT NOT NULL, locale TEXT NOT NULL, value TEXT NOT NULL)",
            "CREATE TABLE prices (\
                id TEXT PRIMARY KEY, variant_id TEXT NOT NULL, price_list_id TEXT NULL, channel_id TEXT NULL, \
                channel_slug TEXT NULL, currency_code TEXT NOT NULL, region_id TEXT NULL, amount_decimal TEXT NOT NULL, \
                compare_at_amount_decimal TEXT NULL, amount INTEGER NULL, compare_at_amount INTEGER NULL, \
                min_quantity INTEGER NULL, max_quantity INTEGER NULL\
             )",
            "CREATE TABLE product_variant_translations (id TEXT PRIMARY KEY, variant_id TEXT NOT NULL, locale TEXT NOT NULL, title TEXT NULL)",
            "CREATE TABLE inventory_items (\
                id TEXT PRIMARY KEY, variant_id TEXT NOT NULL, sku TEXT NULL, requires_shipping BOOLEAN NOT NULL, \
                metadata TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE inventory_levels (\
                id TEXT PRIMARY KEY, inventory_item_id TEXT NOT NULL, location_id TEXT NOT NULL, stocked_quantity INTEGER NOT NULL, \
                reserved_quantity INTEGER NOT NULL, incoming_quantity INTEGER NOT NULL, low_stock_threshold INTEGER NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE product_tags (product_id TEXT NOT NULL, term_id TEXT NOT NULL, tenant_id TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(product_id, term_id))",
            "CREATE TABLE product_field_definitions (\
                id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, field_key TEXT NOT NULL, field_type TEXT NOT NULL, \
                label TEXT NOT NULL, description TEXT NULL, is_localized BOOLEAN NOT NULL, is_required BOOLEAN NOT NULL, \
                default_value TEXT NULL, validation TEXT NULL, position INTEGER NOT NULL, is_active BOOLEAN NOT NULL, \
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
        ] {
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("product copy fixture schema");
        }
        let manager = SchemaManager::new(&database);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration");

        let operator = admin_operator();
        let product_id = Uuid::new_v4();
        let now = "2026-07-16T00:00:00Z".to_string();
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO products (id, tenant_id, status, metadata, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?)"
                    .to_string(),
                vec![
                    product_id.into(),
                    operator.tenant_id.into(),
                    "draft".into(),
                    "{}".into(),
                    now.clone().into(),
                    now.clone().into(),
                ],
            ))
            .await
            .expect("product fixture");
        for (locale, title, handle, description) in [
            (
                "en",
                "Original product",
                "original-product",
                "Original description",
            ),
            (
                "ru",
                "Original Russian product",
                "original-ru-product",
                "Original Russian description",
            ),
        ] {
            database
                .execute(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO product_translations \
                     (id, product_id, tenant_id, locale, title, handle, description) VALUES \
                     (?, ?, ?, ?, ?, ?, ?)"
                        .to_string(),
                    vec![
                        Uuid::new_v4().into(),
                        product_id.into(),
                        operator.tenant_id.into(),
                        locale.to_string().into(),
                        title.to_string().into(),
                        handle.to_string().into(),
                        description.to_string().into(),
                    ],
                ))
                .await
                .expect("product translation fixture");
        }

        let event_bus =
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
        let product_port: Arc<dyn ProductCatalogReadPort> =
            Arc::new(CatalogService::new(database.clone(), event_bus.clone()));
        let runtime = AiHostRuntime::new(
            database.clone(),
            event_bus,
            ModuleRegistry::new(),
            SecretResolverRegistry::builder().build(),
            ProviderEgressPolicy::default(),
            AiProviderTargetCatalog::default(),
        )
        .with_product_catalog_read_port(Some(product_port));
        (runtime, operator, product_id)
    }

    fn admin_operator() -> AiOperatorContext {
        AiOperatorContext {
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            permissions: Rbac::permissions_for_role(&UserRole::Admin)
                .iter()
                .copied()
                .collect(),
            role_slugs: vec!["admin".to_string()],
            preferred_locale: Some("ru".to_string()),
        }
    }

    #[test]
    fn normalize_image_size_accepts_valid_dimensions() {
        assert_eq!(
            normalize_image_size(Some("1024x768".to_string())).unwrap(),
            Some("1024x768".to_string())
        );
    }

    #[test]
    fn normalize_image_size_rejects_invalid_dimensions() {
        assert!(normalize_image_size(Some("wide".to_string())).is_err());
        assert!(normalize_image_size(Some("0x768".to_string())).is_err());
    }

    #[test]
    fn image_provider_request_uses_adapter_validation_and_resolved_locale() {
        let request = build_image_provider_request(
            &AiImageAssetTaskInput {
                prompt: "  editorial hero image  ".to_string(),
                negative_prompt: Some("low contrast".to_string()),
                size: Some(" 1024 x 768 ".to_string()),
                ..Default::default()
            },
            "image-model",
            "ru",
        )
        .unwrap();

        assert_eq!(request.model, "image-model");
        assert_eq!(request.prompt, "editorial hero image");
        assert_eq!(request.negative_prompt.as_deref(), Some("low contrast"));
        assert_eq!(request.size.as_deref(), Some("1024x768"));
        assert_eq!(request.locale.as_deref(), Some("ru"));
    }

    #[test]
    fn image_provider_request_rejects_blank_prompt_before_provider_execution() {
        let error = build_image_provider_request(
            &AiImageAssetTaskInput {
                prompt: "  ".to_string(),
                ..Default::default()
            },
            "image-model",
            "ru",
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("prompt is required for image_asset")
        );
    }

    #[test]
    fn locale_matches_ignores_separator_and_case() {
        assert!(locale_matches("en-us", "en_US"));
        assert!(locale_matches("zh-cn", "zh-CN"));
    }

    #[test]
    fn generated_file_name_uses_sanitized_extension() {
        assert_eq!(
            build_generated_file_name(Some("hero banner"), None, "image/webp"),
            "hero-banner.webp"
        );
    }

    #[test]
    fn normalize_tag_list_trims_and_filters_empty_values() {
        let normalized = normalize_tag_list(&[
            " alpha ".to_string(),
            "".to_string(),
            "beta".to_string(),
            "   ".to_string(),
        ]);
        assert_eq!(normalized, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn generated_blog_content_is_always_created_as_a_draft() {
        let input = AiBlogDraftTaskInput {
            category_id: Some(uuid::Uuid::new_v4()),
            featured_image_url: Some("https://cdn.example.test/hero.webp".to_string()),
            ..Default::default()
        };
        let create = build_blog_draft_create_input(
            &input,
            "ru",
            "Generated title",
            "Generated body",
            Some("Excerpt"),
            Some("generated-title"),
            &["ai".to_string()],
            Some("SEO title"),
            Some("SEO description"),
        )
        .unwrap();

        assert!(!create.publish);
        assert_eq!(create.locale, "ru");
        assert_eq!(create.title, "Generated title");
        assert_eq!(create.tags, vec!["ai".to_string()]);
        assert_eq!(create.category_id, input.category_id);
    }

    #[tokio::test]
    async fn direct_blog_draft_persists_an_unpublished_owner_draft() {
        let (runtime, operator) = blog_runtime().await;
        let request = DirectExecutionRequest {
            task_slug: BLOG_DRAFT_TASK_SLUG.to_string(),
            task_input_json: json!(AiBlogDraftTaskInput {
                source_locale: Some("ru".to_string()),
                source_title: Some("Source title".to_string()),
                source_body: Some("Source body".to_string()),
                assistant_prompt: Some("Summarize the saved draft.".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(BlogDraftEngine),
            stream_emitter: None,
        };

        let result = BlogDraftHandler
            .execute(&runtime, &operator, request)
            .await
            .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Blog);
        assert_eq!(result.metadata["operation"], json!("create_draft"));
        assert_eq!(result.traces.len(), 1);
        assert_eq!(result.traces[0].status, "completed");
        assert_eq!(
            result.traces[0].output_payload.as_ref().unwrap()["post"]["status"],
            json!("draft")
        );
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Draft saved for editorial review.")
        );
    }

    #[tokio::test]
    async fn direct_image_asset_persists_media_and_localized_owner_translation() {
        let storage_dir =
            std::env::temp_dir().join(format!("rustok-ai-image-test-{}", Uuid::new_v4()));
        let storage = StorageRuntime::from_config(&StorageConfig {
            driver: StorageDriver::Local,
            local: LocalStorageConfig {
                base_dir: storage_dir.to_string_lossy().into_owned(),
                base_url: "https://assets.example.test/media".to_string(),
                fsync: false,
            },
            ..Default::default()
        })
        .await
        .expect("media fixture storage");
        let (runtime, operator) = media_runtime(storage).await;
        let captured_request = Arc::new(Mutex::new(None));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_media::IMAGE_ASSET_TASK_SLUG.to_string(),
            task_input_json: json!(AiImageAssetTaskInput {
                prompt: "  editorial hero image  ".to_string(),
                negative_prompt: Some("low contrast".to_string()),
                title: Some("Editorial hero".to_string()),
                alt_text: Some("A localized editorial hero".to_string()),
                caption: Some("Generated for the article header".to_string()),
                file_name: Some("editorial-hero".to_string()),
                size: Some("1024x768".to_string()),
                assistant_prompt: Some("Summarize the saved asset.".to_string()),
            }),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(ImageAssetEngine {
                captured_request: Arc::clone(&captured_request),
            }),
            stream_emitter: None,
        };

        let result = MediaImageAssetHandler
            .execute(&runtime, &operator, request)
            .await
            .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Media);
        assert_eq!(result.traces.len(), 1);
        assert_eq!(result.traces[0].status, "completed");
        assert_eq!(
            result.traces[0].output_payload.as_ref().unwrap()["image_generation"]["size"],
            json!("1024x768")
        );
        assert_eq!(
            result.metadata["media_item"]["mime_type"],
            json!("image/png")
        );
        assert_eq!(result.metadata["translation"]["locale"], json!("ru"));
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Image saved to the media library.")
        );

        let provider_request = captured_request
            .lock()
            .expect("captured image provider request")
            .clone()
            .expect("image provider invocation");
        assert_eq!(provider_request.prompt, "editorial hero image");
        assert_eq!(
            provider_request.negative_prompt.as_deref(),
            Some("low contrast")
        );
        assert_eq!(provider_request.size.as_deref(), Some("1024x768"));
        assert_eq!(provider_request.locale.as_deref(), Some("ru"));

        std::fs::remove_dir_all(&storage_dir).expect("media fixture storage cleanup");
    }

    #[tokio::test]
    async fn direct_product_copy_updates_only_the_requested_locale_through_catalog_owner() {
        let (runtime, operator, product_id) = product_runtime().await;
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_product::PRODUCT_COPY_TASK_SLUG.to_string(),
            task_input_json: json!(AiProductCopyTaskInput {
                product_id,
                source_locale: Some("en".to_string()),
                copy_instructions: Some("Adapt for the Russian catalog.".to_string()),
                assistant_prompt: Some("Summarize the saved product copy.".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(ProductCopyEngine),
            stream_emitter: None,
        };

        let result = ProductCopyHandler
            .execute(&runtime, &operator, request)
            .await
            .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Commerce);
        assert_eq!(result.metadata["target_locale"], json!("ru"));
        assert_eq!(result.metadata["source_locale"], json!("en"));
        assert_eq!(result.traces[0].status, "completed");
        assert_eq!(
            result.traces[0].output_payload.as_ref().unwrap()["target_translation"]["title"],
            json!("Generated product")
        );
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Product copy saved for the selected locale.")
        );

        let catalog = rustok_product::CatalogService::new(runtime.db_clone(), runtime.event_bus());
        let persisted = catalog
            .get_product(operator.tenant_id, product_id)
            .await
            .expect("catalog owner persisted generated copy");
        let english = persisted
            .translations
            .iter()
            .find(|translation| translation.locale == "en")
            .expect("preserved English translation");
        let russian = persisted
            .translations
            .iter()
            .find(|translation| translation.locale == "ru")
            .expect("updated Russian translation");
        assert_eq!(english.title, "Original product");
        assert_eq!(english.handle, "original-product");
        assert_eq!(russian.title, "Generated product");
        assert_eq!(russian.handle, "original-ru-product");
        assert_eq!(
            russian.description.as_deref(),
            Some("Generated description")
        );
    }

    #[tokio::test]
    async fn direct_product_attributes_returns_review_only_suggestions_without_product_write() {
        let (runtime, operator, product_id) = product_runtime().await;
        let product = CatalogService::new(runtime.db_clone(), runtime.event_bus())
            .get_product(operator.tenant_id, product_id)
            .await
            .expect("catalog owner product projection");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime.with_product_catalog_read_port(Some(Arc::new(
            RecordingProductCatalogReadPort {
                calls: Arc::clone(&calls),
                response: Ok(product),
                delay: None,
            },
        )));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_product::PRODUCT_ATTRIBUTES_TASK_SLUG.to_string(),
            task_input_json: json!(AiProductAttributesTaskInput {
                product_id,
                category_slug: Some("apparel".to_string()),
                source_title: Some("Original product".to_string()),
                source_description: Some("Original description".to_string()),
                copy_instructions: Some("Suggest catalog attributes.".to_string()),
                assistant_prompt: Some("Summarize the suggestions.".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(ProductAttributesEngine),
            stream_emitter: None,
        };

        let result = ProductAttributesHandler
            .execute(&runtime, &operator, request)
            .await
            .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Commerce);
        assert_eq!(result.metadata["review_required"], json!(true));
        assert_eq!(result.metadata["persistence"], json!("none"));
        assert_eq!(
            result.metadata["product_context"]["source"],
            json!("owner_port")
        );
        assert_eq!(
            result.metadata["product_context"]["catalog_enrichment"],
            json!("applied")
        );
        assert_eq!(
            result.metadata["suggested_attributes"]["flex_attributes"][0]["key"],
            json!("fabric_weight")
        );
        assert_eq!(result.traces[0].status, "completed");
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Product attributes are ready for review.")
        );

        let catalog = rustok_product::CatalogService::new(runtime.db_clone(), runtime.event_bus());
        let persisted = catalog
            .get_product(operator.tenant_id, product_id)
            .await
            .expect("catalog owner remains the only product writer");
        let russian = persisted
            .translations
            .iter()
            .find(|translation| translation.locale == "ru")
            .expect("preserved Russian translation");
        assert_eq!(russian.title, "Original Russian product");
        assert_eq!(russian.handle, "original-ru-product");

        let calls = calls.lock().expect("product catalog read calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.product_id, product_id);
        assert_eq!(calls[0].1.locale.as_deref(), Some("ru"));
        assert_eq!(calls[0].0.tenant_id, operator.tenant_id.to_string());
        assert_eq!(calls[0].0.deadline_ms, Some(3_000));
    }

    #[tokio::test]
    async fn direct_product_attributes_degrades_when_catalog_port_is_unavailable() {
        let (runtime, operator, product_id) = product_runtime().await;
        let runtime = runtime.with_product_catalog_read_port(Some(Arc::new(
            RecordingProductCatalogReadPort {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Err(PortError::unavailable(
                    "product.remote_unavailable",
                    "product catalog adapter is unavailable",
                )),
                delay: None,
            },
        )));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_product::PRODUCT_ATTRIBUTES_TASK_SLUG.to_string(),
            task_input_json: json!(AiProductAttributesTaskInput {
                product_id,
                source_title: Some("Prompt-only product".to_string()),
                source_description: Some("Prompt-only context".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(ProductAttributesEngine),
            stream_emitter: None,
        };

        let result = ProductAttributesHandler
            .execute(&runtime, &operator, request)
            .await
            .expect("advisory generation must survive an unavailable catalog port");

        assert_eq!(result.metadata["review_required"], json!(true));
        assert_eq!(result.metadata["persistence"], json!("none"));
        assert_eq!(
            result.metadata["product_context"]["source"],
            json!("degraded")
        );
        assert_eq!(
            result.metadata["product_context"]["catalog_enrichment"],
            json!("skipped")
        );
        assert_eq!(
            result.metadata["product_context"]["errors"][0]["code"],
            json!("product.remote_unavailable")
        );
    }

    #[tokio::test]
    async fn direct_product_attributes_degrades_when_catalog_port_exceeds_its_deadline() {
        let (runtime, operator, product_id) = product_runtime().await;
        let product = CatalogService::new(runtime.db_clone(), runtime.event_bus())
            .get_product(operator.tenant_id, product_id)
            .await
            .expect("catalog owner product projection");
        let runtime = runtime.with_product_catalog_read_port(Some(Arc::new(
            RecordingProductCatalogReadPort {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Ok(product),
                delay: Some(std::time::Duration::from_secs(4)),
            },
        )));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_product::PRODUCT_ATTRIBUTES_TASK_SLUG.to_string(),
            task_input_json: json!(AiProductAttributesTaskInput {
                product_id,
                source_title: Some("Prompt-only product".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("ru".to_string()),
            resolved_locale: "ru".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(ProductAttributesEngine),
            stream_emitter: None,
        };

        let result = ProductAttributesHandler
            .execute(&runtime, &operator, request)
            .await
            .expect("advisory generation must survive a catalog-port timeout");

        assert_eq!(
            result.metadata["product_context"]["errors"][0]["kind"],
            json!("deadline_exceeded")
        );
        assert_eq!(
            result.metadata["product_context"]["errors"][0]["code"],
            json!("ai_product.catalog_read_port_deadline_exceeded")
        );
        assert_eq!(result.metadata["review_required"], json!(true));
        assert_eq!(result.metadata["persistence"], json!("none"));
    }

    #[tokio::test]
    async fn direct_order_analytics_is_advisory_and_does_not_persist() {
        let (runtime, operator) = blog_runtime().await;
        let order_id = Uuid::new_v4();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime.with_order_status_port(Some(Arc::new(RecordingOrderStatusPort {
            calls: Arc::clone(&calls),
            response: Ok(OrderStatusSnapshot {
                order_id,
                status: "pending".to_string(),
                paid: false,
                shipped: false,
                delivered: false,
                total_amount: Decimal::new(12_500, 2),
            }),
            delay: None,
        })));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_order::ORDER_ANALYTICS_TASK_SLUG.to_string(),
            task_input_json: json!(AiOrderAnalyticsTaskInput {
                order_ids: vec![order_id],
                focus: Some("shipping risk".to_string()),
                assistant_prompt: Some("Summarize the analytics.".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("en".to_string()),
            resolved_locale: "en".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(OrderAnalyticsEngine),
            stream_emitter: None,
        };

        let result = OrderAnalyticsHandler
            .execute(&runtime, &operator, request)
            .await
            .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Orders);
        assert_eq!(result.metadata["review_required"], json!(true));
        assert_eq!(result.metadata["persistence"], json!("none"));
        assert_eq!(
            result.metadata["order_status_context"]["source"],
            json!("owner_port")
        );
        assert_eq!(
            result.metadata["order_status_context"]["snapshots"][0]["order_id"],
            json!(order_id)
        );
        assert_eq!(
            result.metadata["order_analytics"]["risk_flags"][0],
            json!("address_mismatch")
        );
        assert!(!result.traces[0].sensitive);
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Order analytics are ready for review.")
        );
        let calls = calls.lock().expect("order status calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.order_id, order_id);
        assert_eq!(calls[0].0.deadline_ms, Some(3_000));
        assert_eq!(calls[0].0.tenant_id, operator.tenant_id.to_string());
    }

    #[tokio::test]
    async fn direct_order_operations_are_review_only_and_sensitive() {
        let (runtime, operator) = blog_runtime().await;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime.with_order_status_port(Some(Arc::new(RecordingOrderStatusPort {
            calls: Arc::clone(&calls),
            response: Err(PortError::unavailable(
                "order.remote_unavailable",
                "order status adapter is unavailable",
            )),
            delay: None,
        })));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_order::ORDER_OPS_ASSISTANT_TASK_SLUG.to_string(),
            task_input_json: json!(AiOrderOpsAssistantTaskInput {
                order_id: Uuid::new_v4(),
                recommended_action: Some("contact_customer".to_string()),
                context: Some("Address format differs from prior orders.".to_string()),
                assistant_prompt: Some("Summarize the operation suggestion.".to_string()),
            }),
            requested_locale: Some("en".to_string()),
            resolved_locale: "en".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(OrderOpsAssistantEngine),
            stream_emitter: None,
        };

        let result = OrderOpsAssistantHandler
            .execute(&runtime, &operator, request)
            .await
            .unwrap();

        assert_eq!(result.execution_target, DirectExecutionTarget::Orders);
        assert_eq!(result.metadata["review_required"], json!(true));
        assert_eq!(result.metadata["persistence"], json!("none"));
        assert_eq!(
            result.metadata["order_status_context"]["source"],
            json!("degraded")
        );
        assert_eq!(
            result.metadata["order_status_context"]["errors"][0]["kind"],
            json!("unavailable")
        );
        assert_eq!(
            result.metadata["order_status_context"]["errors"][0]["code"],
            json!("order.remote_unavailable")
        );
        assert_eq!(
            result.metadata["order_ops_assistant"]["recommended_action"],
            json!("contact_customer")
        );
        assert!(result.traces[0].sensitive);
        assert_eq!(
            result.appended_messages[0].content.as_deref(),
            Some("Order operation is ready for review.")
        );
        assert_eq!(calls.lock().expect("order status calls lock").len(), 1);
    }

    #[tokio::test]
    async fn direct_order_analytics_degrades_when_the_status_port_exceeds_its_deadline() {
        let (runtime, operator) = blog_runtime().await;
        let order_id = Uuid::new_v4();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime.with_order_status_port(Some(Arc::new(RecordingOrderStatusPort {
            calls: Arc::clone(&calls),
            response: Ok(OrderStatusSnapshot {
                order_id,
                status: "pending".to_string(),
                paid: false,
                shipped: false,
                delivered: false,
                total_amount: Decimal::new(12_500, 2),
            }),
            delay: Some(std::time::Duration::from_secs(4)),
        })));
        let request = DirectExecutionRequest {
            task_slug: rustok_ai_order::ORDER_ANALYTICS_TASK_SLUG.to_string(),
            task_input_json: json!(AiOrderAnalyticsTaskInput {
                order_ids: vec![order_id],
                focus: Some("shipping risk".to_string()),
                assistant_prompt: Some("Summarize the analytics.".to_string()),
                ..Default::default()
            }),
            requested_locale: Some("en".to_string()),
            resolved_locale: "en".to_string(),
            system_prompt: None,
            provider_config: provider_config(),
            provider: Arc::new(OrderAnalyticsEngine),
            stream_emitter: None,
        };

        let result = OrderAnalyticsHandler
            .execute(&runtime, &operator, request)
            .await
            .expect("generation remains advisory after a status-port timeout");

        assert_eq!(result.metadata["review_required"], json!(true));
        assert_eq!(result.metadata["persistence"], json!("none"));
        assert_eq!(
            result.metadata["order_status_context"]["source"],
            json!("degraded")
        );
        assert_eq!(
            result.metadata["order_status_context"]["errors"][0]["kind"],
            json!("deadline_exceeded")
        );
        assert_eq!(
            result.metadata["order_status_context"]["errors"][0]["code"],
            json!("ai_order.status_port_deadline_exceeded")
        );
        assert_eq!(calls.lock().expect("order status calls lock").len(), 1);
    }

    #[test]
    fn core_defaults_do_not_include_domain_handlers() {
        let registry = super::DirectExecutionRegistry::with_core_defaults();
        assert!(registry.handler("alloy_code").is_some());

        for vertical in media_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_some(),
                "missing registered handler for media task `{}`",
                vertical.task_slug
            );
        }

        for vertical in content_ai_verticals() {
            assert!(registry.handler(vertical.task_slug).is_none());
        }
        for vertical in product_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_none(),
                "core defaults must not register product task `{}`",
                vertical.task_slug
            );
        }
        for vertical in order_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_none(),
                "core defaults must not register order task `{}`",
                vertical.task_slug
            );
        }
    }

    #[test]
    fn defaults_include_domain_handlers() {
        let registry = super::DirectExecutionRegistry::with_defaults();
        assert!(registry.handler("alloy_code").is_some());
        assert!(registry.handler(BLOG_DRAFT_TASK_SLUG).is_some());

        for vertical in media_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_some(),
                "missing registered handler for media task `{}`",
                vertical.task_slug
            );
        }

        for vertical in content_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_some(),
                "missing registered handler for content task `{}`",
                vertical.task_slug
            );
        }
        for vertical in product_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_some(),
                "missing registered handler for product task `{}`",
                vertical.task_slug
            );
        }
        for vertical in order_ai_verticals() {
            assert!(
                registry.handler(vertical.task_slug).is_some(),
                "missing registered handler for order task `{}`",
                vertical.task_slug
            );
        }
    }
}
