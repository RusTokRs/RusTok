use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audience::ForumAudienceConstraints;

/// Marker for a concrete Forum audience scope. One generic policy struct
/// replaces 4× `ForumCategory*AudiencePolicy` copy-paste (P2#2 ideal).
pub trait AudienceScope: Send + Sync + 'static {
    const NAME: &'static str;
    /// Human-readable owner table prefix, e.g. `forum_category_audience`
    const TABLE_PREFIX: &'static str;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AudienceLayer<S: AudienceScope> {
    pub subject_id: Uuid,
    pub constraints: ForumAudienceConstraints,
    #[serde(skip)]
    #[schema(value_type = String)]
    _marker: PhantomData<S>,
}

impl<S: AudienceScope> AudienceLayer<S> {
    pub fn new(subject_id: Uuid, constraints: ForumAudienceConstraints) -> Self {
        Self {
            subject_id,
            constraints,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AudiencePolicy<S: AudienceScope> {
    pub subject_id: Uuid,
    pub configured_constraints: Option<ForumAudienceConstraints>,
    /// Root → leaf non-empty layers, conjunction (child cannot broaden parent).
    pub effective_layers: Vec<AudienceLayer<S>>,
    #[serde(skip)]
    #[schema(value_type = String)]
    _marker: PhantomData<S>,
}

impl<S: AudienceScope> AudiencePolicy<S> {
    pub(super) fn new(
        subject_id: Uuid,
        configured_constraints: Option<ForumAudienceConstraints>,
        effective_layers: Vec<AudienceLayer<S>>,
    ) -> Self {
        Self {
            subject_id,
            configured_constraints,
            effective_layers,
            _marker: PhantomData,
        }
    }

    pub fn subject_id(&self) -> Uuid {
        self.subject_id
    }
    pub fn is_inherited_only(&self) -> bool {
        self.configured_constraints.is_none()
    }
    pub fn effective_constraints(&self) -> Option<&ForumAudienceConstraints> {
        self.effective_layers.last().map(|l| &l.constraints)
    }
}

// Concrete scopes — one line per former copy-paste module
#[derive(Clone, Debug)]
pub struct CategoryScope;
impl AudienceScope for CategoryScope {
    const NAME: &'static str = "category";
    const TABLE_PREFIX: &'static str = "forum_category_audience";
}

#[derive(Clone, Debug)]
pub struct CategoryModerationScope;
impl AudienceScope for CategoryModerationScope {
    const NAME: &'static str = "category_moderation";
    const TABLE_PREFIX: &'static str = "forum_category_moderation_audience";
}

#[derive(Clone, Debug)]
pub struct CategoryTopicCreateScope;
impl AudienceScope for CategoryTopicCreateScope {
    const NAME: &'static str = "category_topic_create";
    const TABLE_PREFIX: &'static str = "forum_category_topic_create_audience";
}

#[derive(Clone, Debug)]
pub struct CategoryReplyCreateScope;
impl AudienceScope for CategoryReplyCreateScope {
    const NAME: &'static str = "category_reply_create";
    const TABLE_PREFIX: &'static str = "forum_category_reply_create_audience";
}

#[derive(Clone, Debug)]
pub struct TopicScope;
impl AudienceScope for TopicScope {
    const NAME: &'static str = "topic";
    const TABLE_PREFIX: &'static str = "forum_topic_audience";
}

#[derive(Clone, Debug)]
pub struct TopicReplyCreateScope;
impl AudienceScope for TopicReplyCreateScope {
    const NAME: &'static str = "topic_reply_create";
    const TABLE_PREFIX: &'static str = "forum_topic_reply_create_audience";
}

// Backwards-compatible type aliases — existing transports keep importing
// `ForumCategoryAudiencePolicy` but now it is a single generic instantiation.
pub type ForumCategoryAudiencePolicyGeneric = AudiencePolicy<CategoryScope>;
pub type ForumCategoryModerationAudiencePolicyGeneric = AudiencePolicy<CategoryModerationScope>;
pub type ForumCategoryTopicCreateAudiencePolicyGeneric = AudiencePolicy<CategoryTopicCreateScope>;
pub type ForumCategoryReplyCreateAudiencePolicyGeneric = AudiencePolicy<CategoryReplyCreateScope>;
pub type ForumTopicAudiencePolicyGeneric = AudiencePolicy<TopicScope>;
pub type ForumTopicReplyCreateAudiencePolicyGeneric = AudiencePolicy<TopicReplyCreateScope>;

pub type ForumCategoryAudienceLayerGeneric = AudienceLayer<CategoryScope>;
