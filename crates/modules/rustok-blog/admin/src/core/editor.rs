use crate::model::{BlogPostDetail, BlogPostDraft};

/// Stable owner of an editable snapshot. Credentials may rotate without
/// changing this identity; actor/tenant switches start a different editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorPrincipal {
    pub actor_id: Option<String>,
    pub tenant: Option<String>,
}

/// Identity of one editor operation. Navigation/principal changes replace this
/// token, so an earlier response cannot overwrite the newly selected snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorRequest {
    id: uuid::Uuid,
    entity_id: Option<String>,
    pub locale: String,
}

impl EditorRequest {
    pub fn new(entity_id: Option<String>, locale: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            entity_id,
            locale,
        }
    }

    pub fn is_current(&self, current: Option<&Self>) -> bool {
        current == Some(self)
    }

    pub fn accepts_post(&self, current: Option<&Self>, post: &BlogPostDetail) -> bool {
        self.is_current(current)
            && self.entity_id.as_ref().is_none_or(|id| id == &post.id)
            && self.locale == post.requested_locale
    }
}

/// One authoritative editor snapshot. Acknowledgement consumes the predecessor
/// revision immediately; a failed independent reload cannot enable another write.
#[derive(Clone, Debug, Default)]
pub enum PostEditorSession {
    #[default]
    Creating,
    CreatingPending(Box<BlogPostDraft>),
    Loaded(Box<BlogPostDetail>),
    Acknowledged {
        post_id: String,
    },
    ReloadFailed {
        post_id: String,
    },
}
impl PostEditorSession {
    pub fn post_id(&self) -> Option<&str> {
        match self {
            Self::Creating | Self::CreatingPending(_) => None,
            Self::Loaded(post) => Some(&post.id),
            Self::Acknowledged { post_id } | Self::ReloadFailed { post_id } => Some(post_id),
        }
    }
    pub fn snapshot(&self) -> Option<&BlogPostDetail> {
        match self {
            Self::Loaded(post) => Some(post),
            _ => None,
        }
    }
    pub fn version(&self) -> Option<i32> {
        self.snapshot().map(|post| post.version)
    }
    pub fn pending_creation(&self) -> Option<&BlogPostDraft> {
        match self {
            Self::CreatingPending(draft) => Some(draft),
            _ => None,
        }
    }

    /// Retain the entire request before IO. A transport error cannot prove that
    /// a previous attempt did not commit, even if current authorization fails.
    pub fn retain_creation(&mut self, draft: BlogPostDraft) -> BlogPostDraft {
        if let Some(pending) = self.pending_creation() {
            return pending.clone();
        }
        *self = Self::CreatingPending(Box::new(draft.clone()));
        draft
    }
    pub fn can_save(&self) -> bool {
        !self.awaiting_reload()
    }
    pub fn reload_notice_key(&self) -> Option<&'static str> {
        matches!(self, Self::ReloadFailed { .. }).then_some("blog.editor.savedReloadFailed")
    }
    pub fn awaiting_reload(&self) -> bool {
        matches!(self, Self::Acknowledged { .. } | Self::ReloadFailed { .. })
    }
}
