use rustok_api::{has_any_effective_permission, Permission};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlogPostMutation {
    Create,
    Update,
    Delete,
    Publish,
    Unpublish,
    Archive,
}

impl BlogPostMutation {
    pub(crate) const fn required_permission(self) -> Permission {
        match self {
            Self::Create => Permission::BLOG_POSTS_CREATE,
            Self::Update | Self::Archive => Permission::BLOG_POSTS_UPDATE,
            Self::Delete => Permission::BLOG_POSTS_DELETE,
            Self::Publish | Self::Unpublish => Permission::BLOG_POSTS_PUBLISH,
        }
    }

    pub(crate) const fn denied_message(self) -> &'static str {
        match self {
            Self::Create => "Permission denied: blog_posts:create required",
            Self::Update | Self::Archive => {
                "Permission denied: blog_posts:update required"
            }
            Self::Delete => "Permission denied: blog_posts:delete required",
            Self::Publish | Self::Unpublish => {
                "Permission denied: blog_posts:publish required"
            }
        }
    }
}

pub(crate) fn can_execute_blog_post_mutation(
    permissions: &[Permission],
    mutation: BlogPostMutation,
) -> bool {
    has_any_effective_permission(permissions, &[mutation.required_permission()])
}
