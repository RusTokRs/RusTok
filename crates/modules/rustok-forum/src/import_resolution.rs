use std::collections::{BTreeMap, BTreeSet};

use rustok_content::normalize_locale_code;
use thiserror::Error;
use uuid::Uuid;

use crate::import_inspection::{
    ForumImportDependencyDisposition, ForumImportDependencyIssue, ForumImportDependencyRelation,
    NodebbForumImportInspection,
};
use crate::import_mapping::{
    FORUM_IMPORT_SOURCE_NODEBB, ForumImportCategoryCandidate, ForumImportEntityKind,
    ForumImportExternalRef, ForumImportPostCandidate, ForumImportPostRole,
    ForumImportTopicCandidate, MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH,
};

pub const MAX_FORUM_IMPORT_RESOLUTION_BINDINGS_PER_BATCH: usize =
    MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH * 2;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ForumImportTargetIdentityKind {
    Category,
    Topic,
    Reply,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumImportIdentityBinding {
    pub source: ForumImportExternalRef,
    pub target_kind: ForumImportTargetIdentityKind,
    pub target_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct ForumImportApplicationResolutionRequest {
    pub tenant_id: Uuid,
    pub locale: String,
    pub inspection: NodebbForumImportInspection,
    pub bindings: Vec<ForumImportIdentityBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumResolvedImportAuthor {
    pub source: ForumImportExternalRef,
    pub user_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumResolvedImportCategory {
    pub source: ForumImportExternalRef,
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub position: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumResolvedImportTopic {
    pub source: ForumImportExternalRef,
    pub id: Uuid,
    pub category_id: Uuid,
    pub author: Option<ForumResolvedImportAuthor>,
    pub title: String,
    pub slug: Option<String>,
    pub body_source: ForumImportExternalRef,
    pub body: String,
    pub created_at_ms: Option<i64>,
    pub is_pinned: bool,
    pub is_locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumResolvedImportReply {
    pub source: ForumImportExternalRef,
    pub id: Uuid,
    pub topic_id: Uuid,
    pub author: Option<ForumResolvedImportAuthor>,
    pub body: String,
    pub created_at_ms: Option<i64>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumResolvedImportApplicationBatch {
    pub tenant_id: Uuid,
    pub locale: String,
    pub categories: Vec<ForumResolvedImportCategory>,
    pub topics: Vec<ForumResolvedImportTopic>,
    pub replies: Vec<ForumResolvedImportReply>,
}

#[derive(Debug, Error)]
pub enum ForumImportResolutionError {
    #[error("Forum import resolution requires a non-nil tenant id")]
    NilTenantId,
    #[error("Forum import resolution locale is invalid: {locale}")]
    InvalidLocale { locale: String },
    #[error("Forum import resolution exceeds {max} source candidates: {actual}")]
    TooManyCandidates { max: usize, actual: usize },
    #[error("Forum import resolution exceeds {max} identity bindings: {actual}")]
    TooManyBindings { max: usize, actual: usize },
    #[error("Forum import resolution repeats source candidate {source:?}")]
    DuplicateCandidateSource { source: ForumImportExternalRef },
    #[error("Forum import resolution contains duplicate binding for {source:?}")]
    DuplicateBinding { source: ForumImportExternalRef },
    #[error("Forum import resolution binding for {source:?} has nil target id")]
    NilBindingTarget { source: ForumImportExternalRef },
    #[error(
        "Forum import resolution maps multiple {kind:?} sources onto target {target_id}: {first:?} and {second:?}"
    )]
    TargetIdentityCollision {
        kind: ForumImportTargetIdentityKind,
        target_id: Uuid,
        first: ForumImportExternalRef,
        second: ForumImportExternalRef,
    },
    #[error("Forum import resolution is missing {expected:?} binding for {source:?}")]
    MissingBinding {
        source: ForumImportExternalRef,
        expected: ForumImportTargetIdentityKind,
    },
    #[error(
        "Forum import resolution binding for {source:?} targets {actual:?}, expected {expected:?}"
    )]
    BindingKindMismatch {
        source: ForumImportExternalRef,
        expected: ForumImportTargetIdentityKind,
        actual: ForumImportTargetIdentityKind,
    },
    #[error("Forum import resolution contains unused binding for {source:?}")]
    UnexpectedBinding { source: ForumImportExternalRef },
    #[error(
        "Forum import resolution requires NodeBB source namespace for {reference:?}, got {actual}"
    )]
    SourceNamespaceMismatch {
        reference: ForumImportExternalRef,
        actual: String,
    },
    #[error(
        "Forum import resolution reference {reference:?} has kind {actual:?}, expected {expected:?}"
    )]
    SourceKindMismatch {
        reference: ForumImportExternalRef,
        expected: ForumImportEntityKind,
        actual: ForumImportEntityKind,
    },
    #[error(
        "Forum import dependency remains unresolved for application: {relation:?}/{disposition:?} owner={owner:?} target={target:?}"
    )]
    UnresolvedDependency {
        owner: ForumImportExternalRef,
        relation: ForumImportDependencyRelation,
        target: ForumImportExternalRef,
        disposition: ForumImportDependencyDisposition,
    },
    #[error(
        "Forum import application batch requires in-batch {relation:?}: owner={owner:?} target={target:?}"
    )]
    CrossBatchDependency {
        owner: ForumImportExternalRef,
        relation: ForumImportDependencyRelation,
        target: ForumImportExternalRef,
    },
    #[error("Forum import category parent cycle reaches {source:?}")]
    CategoryCycle { source: ForumImportExternalRef },
    #[error("Forum import post role is unresolved for {source:?}")]
    UnresolvedPostRole { source: ForumImportExternalRef },
    #[error("Forum import topic {source:?} has no explicit main-post body source")]
    MissingTopicBodySource { source: ForumImportExternalRef },
    #[error("Forum import topic {topic:?} body post {post:?} is absent from the bounded batch")]
    TopicBodyPostMissing {
        topic: ForumImportExternalRef,
        post: ForumImportExternalRef,
    },
    #[error("Forum import topic {topic:?} body post {post:?} is not classified as topic_body")]
    TopicBodyRoleMismatch {
        topic: ForumImportExternalRef,
        post: ForumImportExternalRef,
    },
    #[error("Forum import topic {topic:?} body post {post:?} is marked deleted")]
    DeletedTopicBody {
        topic: ForumImportExternalRef,
        post: ForumImportExternalRef,
    },
    #[error("Forum import topic {topic:?} and body post {post:?} resolve to different authors")]
    TopicBodyAuthorMismatch {
        topic: ForumImportExternalRef,
        post: ForumImportExternalRef,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ForumImportApplicationResolver;

impl ForumImportApplicationResolver {
    pub fn resolve_batch(
        &self,
        request: &ForumImportApplicationResolutionRequest,
    ) -> Result<ForumResolvedImportApplicationBatch, ForumImportResolutionError> {
        let locale = validate_request(request)?;
        reject_non_author_dependency_issues(&request.inspection.unresolved_dependencies)?;
        validate_candidate_sources(&request.inspection)?;

        let mut bindings = BindingIndex::new(&request.bindings)?;
        let categories = resolve_categories(&request.inspection, &mut bindings)?;
        let topics = resolve_topics(&request.inspection, &mut bindings)?;
        let replies = resolve_replies(&request.inspection, &mut bindings)?;
        bindings.reject_unused()?;

        Ok(ForumResolvedImportApplicationBatch {
            tenant_id: request.tenant_id,
            locale,
            categories,
            topics,
            replies,
        })
    }
}

type RefKey = (String, &'static str, String);

struct BindingIndex {
    by_source: BTreeMap<RefKey, ForumImportIdentityBinding>,
    used: BTreeSet<RefKey>,
}

impl BindingIndex {
    fn new(bindings: &[ForumImportIdentityBinding]) -> Result<Self, ForumImportResolutionError> {
        let mut by_source = BTreeMap::new();
        let mut by_target =
            BTreeMap::<(ForumImportTargetIdentityKind, Uuid), ForumImportExternalRef>::new();

        for binding in bindings {
            validate_external_ref(&binding.source, binding.source.kind)?;
            if binding.target_id.is_nil() {
                return Err(ForumImportResolutionError::NilBindingTarget {
                    source: binding.source.clone(),
                });
            }
            let key = ref_key(&binding.source);
            if by_source.insert(key, binding.clone()).is_some() {
                return Err(ForumImportResolutionError::DuplicateBinding {
                    source: binding.source.clone(),
                });
            }
            if let Some(first) = by_target.insert(
                (binding.target_kind, binding.target_id),
                binding.source.clone(),
            ) && first != binding.source
            {
                return Err(ForumImportResolutionError::TargetIdentityCollision {
                    kind: binding.target_kind,
                    target_id: binding.target_id,
                    first,
                    second: binding.source.clone(),
                });
            }
        }

        Ok(Self {
            by_source,
            used: BTreeSet::new(),
        })
    }

    fn require(
        &mut self,
        source: &ForumImportExternalRef,
        expected: ForumImportTargetIdentityKind,
    ) -> Result<Uuid, ForumImportResolutionError> {
        let key = ref_key(source);
        let Some(binding) = self.by_source.get(&key) else {
            return Err(ForumImportResolutionError::MissingBinding {
                source: source.clone(),
                expected,
            });
        };
        if binding.target_kind != expected {
            return Err(ForumImportResolutionError::BindingKindMismatch {
                source: source.clone(),
                expected,
                actual: binding.target_kind,
            });
        }
        self.used.insert(key);
        Ok(binding.target_id)
    }

    fn reject_unused(&self) -> Result<(), ForumImportResolutionError> {
        for (key, binding) in &self.by_source {
            if !self.used.contains(key) {
                return Err(ForumImportResolutionError::UnexpectedBinding {
                    source: binding.source.clone(),
                });
            }
        }
        Ok(())
    }
}

fn validate_request(
    request: &ForumImportApplicationResolutionRequest,
) -> Result<String, ForumImportResolutionError> {
    if request.tenant_id.is_nil() {
        return Err(ForumImportResolutionError::NilTenantId);
    }
    let locale = normalize_locale_code(&request.locale).ok_or_else(|| {
        ForumImportResolutionError::InvalidLocale {
            locale: request.locale.clone(),
        }
    })?;

    let candidate_count = request
        .inspection
        .candidates
        .categories
        .len()
        .saturating_add(request.inspection.candidates.topics.len())
        .saturating_add(request.inspection.candidates.posts.len());
    if candidate_count > MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH {
        return Err(ForumImportResolutionError::TooManyCandidates {
            max: MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH,
            actual: candidate_count,
        });
    }
    if request.bindings.len() > MAX_FORUM_IMPORT_RESOLUTION_BINDINGS_PER_BATCH {
        return Err(ForumImportResolutionError::TooManyBindings {
            max: MAX_FORUM_IMPORT_RESOLUTION_BINDINGS_PER_BATCH,
            actual: request.bindings.len(),
        });
    }
    Ok(locale)
}

fn reject_non_author_dependency_issues(
    issues: &[ForumImportDependencyIssue],
) -> Result<(), ForumImportResolutionError> {
    for issue in issues {
        if issue.relation == ForumImportDependencyRelation::AuthorUser
            && issue.disposition == ForumImportDependencyDisposition::ExternalOwnerResolution
        {
            continue;
        }
        return Err(ForumImportResolutionError::UnresolvedDependency {
            owner: issue.owner.clone(),
            relation: issue.relation,
            target: issue.target.clone(),
            disposition: issue.disposition,
        });
    }
    Ok(())
}

fn validate_candidate_sources(
    inspection: &NodebbForumImportInspection,
) -> Result<(), ForumImportResolutionError> {
    let mut all_sources = BTreeSet::new();
    let mut category_sources = BTreeSet::new();
    let mut topic_sources = BTreeSet::new();
    let mut post_sources = BTreeSet::new();
    let mut category_by_key = BTreeMap::new();

    for category in &inspection.candidates.categories {
        validate_external_ref(&category.source, ForumImportEntityKind::Category)?;
        insert_candidate_source(&mut all_sources, &category.source)?;
        category_sources.insert(ref_key(&category.source));
        category_by_key.insert(ref_key(&category.source), category);
        if let Some(parent) = category.parent_source.as_ref() {
            validate_external_ref(parent, ForumImportEntityKind::Category)?;
        }
    }
    for topic in &inspection.candidates.topics {
        validate_external_ref(&topic.source, ForumImportEntityKind::Topic)?;
        insert_candidate_source(&mut all_sources, &topic.source)?;
        topic_sources.insert(ref_key(&topic.source));
        validate_external_ref(&topic.category_source, ForumImportEntityKind::Category)?;
        if let Some(author) = topic.author_source.as_ref() {
            validate_external_ref(author, ForumImportEntityKind::User)?;
        }
        if let Some(body) = topic.body_post_source.as_ref() {
            validate_external_ref(body, ForumImportEntityKind::Post)?;
        }
    }
    for post in &inspection.candidates.posts {
        validate_external_ref(&post.source, ForumImportEntityKind::Post)?;
        insert_candidate_source(&mut all_sources, &post.source)?;
        post_sources.insert(ref_key(&post.source));
        validate_external_ref(&post.topic_source, ForumImportEntityKind::Topic)?;
        if let Some(author) = post.author_source.as_ref() {
            validate_external_ref(author, ForumImportEntityKind::User)?;
        }
        if post.role == ForumImportPostRole::Unresolved {
            return Err(ForumImportResolutionError::UnresolvedPostRole {
                source: post.source.clone(),
            });
        }
    }

    for category in &inspection.candidates.categories {
        if let Some(parent) = category.parent_source.as_ref()
            && !category_sources.contains(&ref_key(parent))
        {
            return Err(ForumImportResolutionError::CrossBatchDependency {
                owner: category.source.clone(),
                relation: ForumImportDependencyRelation::CategoryParent,
                target: parent.clone(),
            });
        }
    }
    validate_category_acyclic(&inspection.candidates.categories, &category_by_key)?;

    for topic in &inspection.candidates.topics {
        if !category_sources.contains(&ref_key(&topic.category_source)) {
            return Err(ForumImportResolutionError::CrossBatchDependency {
                owner: topic.source.clone(),
                relation: ForumImportDependencyRelation::TopicCategory,
                target: topic.category_source.clone(),
            });
        }
        let body_source = topic.body_post_source.as_ref().ok_or_else(|| {
            ForumImportResolutionError::MissingTopicBodySource {
                source: topic.source.clone(),
            }
        })?;
        if !post_sources.contains(&ref_key(body_source)) {
            return Err(ForumImportResolutionError::CrossBatchDependency {
                owner: topic.source.clone(),
                relation: ForumImportDependencyRelation::TopicMainPost,
                target: body_source.clone(),
            });
        }
    }

    for post in &inspection.candidates.posts {
        if !topic_sources.contains(&ref_key(&post.topic_source)) {
            return Err(ForumImportResolutionError::CrossBatchDependency {
                owner: post.source.clone(),
                relation: ForumImportDependencyRelation::PostTopic,
                target: post.topic_source.clone(),
            });
        }
    }

    Ok(())
}

fn insert_candidate_source(
    seen: &mut BTreeSet<RefKey>,
    source: &ForumImportExternalRef,
) -> Result<(), ForumImportResolutionError> {
    if !seen.insert(ref_key(source)) {
        return Err(ForumImportResolutionError::DuplicateCandidateSource {
            source: source.clone(),
        });
    }
    Ok(())
}

fn validate_category_acyclic(
    categories: &[ForumImportCategoryCandidate],
    category_by_key: &BTreeMap<RefKey, &ForumImportCategoryCandidate>,
) -> Result<(), ForumImportResolutionError> {
    for category in categories {
        let mut seen = BTreeSet::new();
        let mut current = category;
        loop {
            let current_key = ref_key(&current.source);
            if !seen.insert(current_key) {
                return Err(ForumImportResolutionError::CategoryCycle {
                    source: current.source.clone(),
                });
            }
            let Some(parent_source) = current.parent_source.as_ref() else {
                break;
            };
            let Some(parent) = category_by_key.get(&ref_key(parent_source)).copied() else {
                break;
            };
            current = parent;
        }
    }
    Ok(())
}

fn resolve_categories(
    inspection: &NodebbForumImportInspection,
    bindings: &mut BindingIndex,
) -> Result<Vec<ForumResolvedImportCategory>, ForumImportResolutionError> {
    inspection
        .candidates
        .categories
        .iter()
        .map(|candidate| resolve_category(candidate, bindings))
        .collect()
}

fn resolve_category(
    candidate: &ForumImportCategoryCandidate,
    bindings: &mut BindingIndex,
) -> Result<ForumResolvedImportCategory, ForumImportResolutionError> {
    let id = bindings.require(&candidate.source, ForumImportTargetIdentityKind::Category)?;
    let parent_id = candidate
        .parent_source
        .as_ref()
        .map(|source| bindings.require(source, ForumImportTargetIdentityKind::Category))
        .transpose()?;
    Ok(ForumResolvedImportCategory {
        source: candidate.source.clone(),
        id,
        parent_id,
        name: candidate.name.clone(),
        description: candidate.description.clone(),
        position: candidate.position,
    })
}

fn resolve_topics(
    inspection: &NodebbForumImportInspection,
    bindings: &mut BindingIndex,
) -> Result<Vec<ForumResolvedImportTopic>, ForumImportResolutionError> {
    inspection
        .candidates
        .topics
        .iter()
        .map(|candidate| resolve_topic(candidate, inspection, bindings))
        .collect()
}

fn resolve_topic(
    candidate: &ForumImportTopicCandidate,
    inspection: &NodebbForumImportInspection,
    bindings: &mut BindingIndex,
) -> Result<ForumResolvedImportTopic, ForumImportResolutionError> {
    let id = bindings.require(&candidate.source, ForumImportTargetIdentityKind::Topic)?;
    let category_id = bindings.require(
        &candidate.category_source,
        ForumImportTargetIdentityKind::Category,
    )?;
    let author = resolve_author(candidate.author_source.as_ref(), bindings)?;
    let body_source = candidate.body_post_source.as_ref().ok_or_else(|| {
        ForumImportResolutionError::MissingTopicBodySource {
            source: candidate.source.clone(),
        }
    })?;
    let body_post = inspection
        .candidates
        .posts
        .iter()
        .find(|post| post.source == *body_source)
        .ok_or_else(|| ForumImportResolutionError::TopicBodyPostMissing {
            topic: candidate.source.clone(),
            post: body_source.clone(),
        })?;
    if body_post.role != ForumImportPostRole::TopicBody
        || body_post.topic_source != candidate.source
    {
        return Err(ForumImportResolutionError::TopicBodyRoleMismatch {
            topic: candidate.source.clone(),
            post: body_post.source.clone(),
        });
    }
    if body_post.deleted {
        return Err(ForumImportResolutionError::DeletedTopicBody {
            topic: candidate.source.clone(),
            post: body_post.source.clone(),
        });
    }
    let body_author = resolve_author(body_post.author_source.as_ref(), bindings)?;
    if author.as_ref().map(|value| value.user_id) != body_author.as_ref().map(|value| value.user_id)
    {
        return Err(ForumImportResolutionError::TopicBodyAuthorMismatch {
            topic: candidate.source.clone(),
            post: body_post.source.clone(),
        });
    }

    Ok(ForumResolvedImportTopic {
        source: candidate.source.clone(),
        id,
        category_id,
        author,
        title: candidate.title.clone(),
        slug: candidate.slug.clone(),
        body_source: body_post.source.clone(),
        body: body_post.body.clone(),
        created_at_ms: candidate.created_at_ms,
        is_pinned: candidate.is_pinned,
        is_locked: candidate.is_locked,
    })
}

fn resolve_replies(
    inspection: &NodebbForumImportInspection,
    bindings: &mut BindingIndex,
) -> Result<Vec<ForumResolvedImportReply>, ForumImportResolutionError> {
    let mut replies = Vec::new();
    for candidate in &inspection.candidates.posts {
        match candidate.role {
            ForumImportPostRole::TopicBody => continue,
            ForumImportPostRole::Reply => replies.push(resolve_reply(candidate, bindings)?),
            ForumImportPostRole::Unresolved => {
                return Err(ForumImportResolutionError::UnresolvedPostRole {
                    source: candidate.source.clone(),
                });
            }
        }
    }
    Ok(replies)
}

fn resolve_reply(
    candidate: &ForumImportPostCandidate,
    bindings: &mut BindingIndex,
) -> Result<ForumResolvedImportReply, ForumImportResolutionError> {
    let id = bindings.require(&candidate.source, ForumImportTargetIdentityKind::Reply)?;
    let topic_id = bindings.require(
        &candidate.topic_source,
        ForumImportTargetIdentityKind::Topic,
    )?;
    let author = resolve_author(candidate.author_source.as_ref(), bindings)?;
    Ok(ForumResolvedImportReply {
        source: candidate.source.clone(),
        id,
        topic_id,
        author,
        body: candidate.body.clone(),
        created_at_ms: candidate.created_at_ms,
        deleted: candidate.deleted,
    })
}

fn resolve_author(
    source: Option<&ForumImportExternalRef>,
    bindings: &mut BindingIndex,
) -> Result<Option<ForumResolvedImportAuthor>, ForumImportResolutionError> {
    source
        .map(|source| {
            Ok(ForumResolvedImportAuthor {
                source: source.clone(),
                user_id: bindings.require(source, ForumImportTargetIdentityKind::User)?,
            })
        })
        .transpose()
}

fn validate_external_ref(
    reference: &ForumImportExternalRef,
    expected: ForumImportEntityKind,
) -> Result<(), ForumImportResolutionError> {
    if reference.source != FORUM_IMPORT_SOURCE_NODEBB {
        return Err(ForumImportResolutionError::SourceNamespaceMismatch {
            reference: reference.clone(),
            actual: reference.source.clone(),
        });
    }
    if reference.kind != expected {
        return Err(ForumImportResolutionError::SourceKindMismatch {
            reference: reference.clone(),
            expected,
            actual: reference.kind,
        });
    }
    Ok(())
}

fn ref_key(reference: &ForumImportExternalRef) -> RefKey {
    (
        reference.source.clone(),
        source_kind_label(reference.kind),
        reference.key.clone(),
    )
}

const fn source_kind_label(kind: ForumImportEntityKind) -> &'static str {
    match kind {
        ForumImportEntityKind::Category => "category",
        ForumImportEntityKind::Topic => "topic",
        ForumImportEntityKind::Post => "post",
        ForumImportEntityKind::User => "user",
    }
}
