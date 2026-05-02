#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Arc;

use alloy::utils::{dynamic_to_json, json_to_dynamic};
use alloy::ScriptRegistry;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use loco_rs::app::AppContext;
use rustok_api::context::infer_user_role_from_permissions;
use rustok_api::loco::transactional_event_bus_from_context;
use rustok_blog::{CreatePostInput, PostService, UpdatePostInput};
use rustok_commerce::{CatalogService, ProductTranslationInput, UpdateProductInput};
use rustok_mcp::alloy_tools::{alloy_validate_script, AlloyMcpState, ValidateScriptRequest};
use rustok_media::{MediaService, UploadInput, UpsertTranslationInput};
use rustok_storage::StorageService;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::model::{
    AiAlloyOperation, AiAlloyTaskInput, AiBlogDraftTaskInput, AiImageAssetTaskInput,
    AiProductCopyTaskInput, AiProviderConfig, ChatMessage, ChatMessageRole, DirectExecutionTarget,
    ProviderChatRequest, ProviderImageRequest, ProviderStreamEmitter, ToolTrace,
};
use crate::provider::ModelProvider;
use crate::service::AiOperatorContext;
use crate::{AiError, AiResult};
use rustok_core::{SecurityContext, CONTENT_FORMAT_MARKDOWN};

pub struct DirectExecutionRequest {
    pub task_slug: String,
    pub task_input_json: Value,
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub system_prompt: Option<String>,
    pub provider_config: AiProviderConfig,
    pub provider: Arc<dyn ModelProvider>,
    pub stream_emitter: Option<ProviderStreamEmitter>,
}

pub struct DirectExecutionResult {
    pub execution_target: DirectExecutionTarget,
    pub appended_messages: Vec<ChatMessage>,
    pub traces: Vec<ToolTrace>,
    pub metadata: Value,
}

#[async_trait]
pub trait DirectTaskHandler: Send + Sync {
    fn task_slug(&self) -> &'static str;

    async fn execute(
        &self,
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult>;
}

#[derive(Default)]
pub struct DirectExecutionRegistry {
    handlers: HashMap<&'static str, Arc<dyn DirectTaskHandler>>,
}

impl DirectExecutionRegistry {
    pub fn with_defaults() -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(AlloyScriptAssistHandler));
        registry.register(Arc::new(MediaImageAssetHandler));
        registry.register(Arc::new(ProductCopyHandler));
        registry.register(Arc::new(BlogDraftHandler));
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
        "alloy_code"
    }

    async fn execute(
        &self,
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiAlloyTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let scoped = alloy::scoped_runtime(app_ctx, operator.tenant_id);
        let started = std::time::Instant::now();

        let (trace_name, operation_payload, summary) = match input.operation {
            AiAlloyOperation::ListScripts => {
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
            AiAlloyOperation::GetScript => {
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
                            "code": script.code,
                            "trigger": script.trigger,
                        }
                    }),
                    format!("Loaded Alloy script `{}`.", script.name),
                )
            }
            AiAlloyOperation::ValidateScript => {
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
            AiAlloyOperation::RunScript => {
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
        "image_asset"
    }

    async fn execute(
        &self,
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiImageAssetTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let prompt = input.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(AiError::Validation(
                "prompt is required for image_asset".to_string(),
            ));
        }

        let started = std::time::Instant::now();
        let provider_image = request
            .provider
            .generate_image(
                &request.provider_config,
                ProviderImageRequest {
                    model: request.provider_config.model.clone(),
                    prompt: prompt.clone(),
                    negative_prompt: input.negative_prompt.clone(),
                    size: normalize_image_size(input.size.clone())?,
                    locale: Some(request.resolved_locale.clone()),
                },
            )
            .await?;

        let file_name = build_generated_file_name(
            input.file_name.as_deref(),
            input.title.as_deref(),
            &provider_image.mime_type,
        );
        let media_service = MediaService::new(app_ctx.db.clone(), storage_from_app_ctx(app_ctx)?);
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
                "provider_kind": request.provider_config.provider_kind.slug(),
                "model": request.provider_config.model,
                "size": normalize_image_size(input.size)?.unwrap_or_else(|| "1024x1024".to_string()),
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GeneratedProductCopy {
    title: Option<String>,
    handle: Option<String>,
    description: Option<String>,
    meta_title: Option<String>,
    meta_description: Option<String>,
}

#[async_trait]
impl DirectTaskHandler for ProductCopyHandler {
    fn task_slug(&self) -> &'static str {
        "product_copy"
    }

    async fn execute(
        &self,
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiProductCopyTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let catalog = CatalogService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(app_ctx),
        );
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
            tool_name: "direct.commerce.product_copy".to_string(),
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GeneratedBlogDraft {
    title: Option<String>,
    slug: Option<String>,
    body: Option<String>,
    excerpt: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
}

#[async_trait]
impl DirectTaskHandler for BlogDraftHandler {
    fn task_slug(&self) -> &'static str {
        "blog_draft"
    }

    async fn execute(
        &self,
        app_ctx: &AppContext,
        operator: &AiOperatorContext,
        request: DirectExecutionRequest,
    ) -> AiResult<DirectExecutionResult> {
        let input: AiBlogDraftTaskInput =
            serde_json::from_value(request.task_input_json.clone()).map_err(AiError::Json)?;
        let started = std::time::Instant::now();
        let service = PostService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(app_ctx),
        );
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
                    CreatePostInput {
                        locale: request.resolved_locale.clone(),
                        title: title.clone(),
                        body: body.clone(),
                        body_format: CONTENT_FORMAT_MARKDOWN.to_string(),
                        content_json: None,
                        excerpt: excerpt.clone(),
                        slug: slug.clone(),
                        publish: false,
                        tags: tags.clone(),
                        category_id: input.category_id,
                        featured_image_url: input.featured_image_url.clone(),
                        seo_title: seo_title.clone(),
                        seo_description: seo_description.clone(),
                        channel_slugs: None,
                        metadata: None,
                    },
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
            tool_name: "direct.blog.generate_draft".to_string(),
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
    product: &rustok_commerce::ProductResponse,
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
    provider: &Arc<dyn ModelProvider>,
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

    let response = provider
        .complete(
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

    let content = response.assistant_message.content.ok_or_else(|| {
        AiError::Provider("provider returned empty content for blog_draft".to_string())
    })?;
    let parsed = parse_json_object_from_text(&content)?;
    serde_json::from_value(parsed).map_err(AiError::Json)
}

#[allow(clippy::too_many_arguments)]
async fn generate_product_copy(
    provider: &Arc<dyn ModelProvider>,
    provider_config: &AiProviderConfig,
    system_prompt: Option<&str>,
    target_locale: &str,
    product: &rustok_commerce::ProductResponse,
    source_translation: &ProductSourceTranslation,
    current_target_translation: Option<&rustok_commerce::ProductTranslationResponse>,
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

    let response = provider
        .complete(
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

    let content = response.assistant_message.content.ok_or_else(|| {
        AiError::Provider("provider returned empty content for product_copy".to_string())
    })?;
    parse_generated_product_copy(&content)
}

fn parse_generated_product_copy(content: &str) -> AiResult<GeneratedProductCopy> {
    let parsed = parse_json_object_from_text(content)?;
    serde_json::from_value(parsed).map_err(AiError::Json)
}

fn parse_json_object_from_text(content: &str) -> AiResult<Value> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(AiError::Provider(
            "provider returned empty JSON payload".to_string(),
        ));
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }

    let start = trimmed.find('{').ok_or_else(|| {
        AiError::Provider("provider response did not contain a JSON object".to_string())
    })?;
    let end = trimmed.rfind('}').ok_or_else(|| {
        AiError::Provider("provider response did not contain a complete JSON object".to_string())
    })?;
    serde_json::from_str::<Value>(&trimmed[start..=end]).map_err(AiError::Json)
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

fn ai_security_context(operator: &AiOperatorContext) -> SecurityContext {
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

fn storage_from_app_ctx(app_ctx: &AppContext) -> AiResult<StorageService> {
    app_ctx.shared_store.get::<StorageService>().ok_or_else(|| {
        AiError::Runtime("StorageService is not registered in AppContext".to_string())
    })
}

fn normalize_image_size(size: Option<String>) -> AiResult<Option<String>> {
    let Some(size) = size
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let (width, height) = size.split_once('x').ok_or_else(|| {
        AiError::Validation("image size must be formatted as WIDTHxHEIGHT".to_string())
    })?;
    let width = width
        .trim()
        .parse::<u32>()
        .map_err(|_| AiError::Validation("image width must be numeric".to_string()))?;
    let height = height
        .trim()
        .parse::<u32>()
        .map_err(|_| AiError::Validation("image height must be numeric".to_string()))?;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(AiError::Validation(
            "image size must stay within 1..=4096 for both dimensions".to_string(),
        ));
    }
    Ok(Some(format!("{width}x{height}")))
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
async fn explain_result(
    provider: &Arc<dyn ModelProvider>,
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
    use super::{
        build_generated_file_name, locale_matches, normalize_image_size, normalize_tag_list,
        parse_generated_product_copy, parse_json_object_from_text,
    };

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
    fn parse_generated_product_copy_accepts_embedded_json() {
        let parsed = parse_generated_product_copy(
            "```json\n{\"title\":\"Titel\",\"description\":\"Beschreibung\"}\n```",
        )
        .unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Titel"));
        assert_eq!(parsed.description.as_deref(), Some("Beschreibung"));
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
    fn parse_json_object_from_text_accepts_embedded_payload() {
        let parsed =
            parse_json_object_from_text("result:\n```json\n{\"title\":\"Hallo\"}\n```").unwrap();
        assert_eq!(parsed["title"], "Hallo");
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
}
