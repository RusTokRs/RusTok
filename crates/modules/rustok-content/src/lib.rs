/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-content
//!
//! Shared content helpers, richtext validation, locale resolution, and cross-domain
//! content orchestration core for RusToK.

pub mod analytics;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod locale;
pub mod migrations;
pub mod module;
pub mod richtext;
#[cfg(feature = "richtext-assets")]
pub mod richtext_assets;
pub mod services;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;

pub use analytics::{ContentCountSnapshot, load_post_stats_snapshot};
pub use dto::{
    CreateTagInput, ListTagsFilter, TagListItem, TagResponse, UpdateTagInput, format_single_error,
    format_validation_errors,
};
pub use entities::{Body, Node, NodeTranslation};
pub use error::{ContentError, ContentResult};
pub use locale::{
    ResolvedLocale, available_locales_from, normalize_locale_code, resolve_by_locale,
    resolve_by_locale_with_fallback,
};
pub use module::ContentModule;
pub use richtext::{
    RichTextError, RichTextErrorCode, RichTextLimits, RichTextProfile, RichTextProfileManifest,
    all_profile_manifests, canonical_json, parse_json, plain_text, profile_from_id, project,
    render_html, validate, validate_and_normalize,
};
pub use services::{
    CanonicalRouteResolver, ContentOrchestrationBridge, ContentOrchestrationService,
    DemotePostToTopicInput, DemotePostToTopicOutput, MergeTopicsInput, MergeTopicsOutput,
    OrchestrationResult, PromoteTopicToPostInput, PromoteTopicToPostOutput, ResolvedContentRoute,
    SharedCanonicalRouteResolver, SplitTopicInput, SplitTopicOutput, normalize_route,
};
pub use state_machine::{Archived, ContentNode, Draft, Published, ToContentStatus};

#[cfg(test)]
mod contract_tests;
