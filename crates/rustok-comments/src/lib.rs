pub mod dto;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod services;

#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_core::permissions::{Action, Permission, Resource};
#[cfg(feature = "server")]
use rustok_core::{MigrationSource, RusToKModule};
#[cfg(feature = "server")]
use sea_orm_migration::MigrationTrait;

pub use dto::{
    CommentListItem, CommentRecord, CommentStatus, CommentThreadDetail, CommentThreadStatus,
    CommentThreadSummary, CreateCommentInput, ListCommentsFilter, UpdateCommentInput,
};
#[cfg(feature = "server")]
pub use entities::*;
#[cfg(feature = "server")]
pub use error::{CommentsError, CommentsResult};
#[cfg(feature = "server")]
pub use services::CommentsService;

#[cfg(feature = "server")]
pub struct CommentsModule;

#[cfg(feature = "server")]
#[async_trait]
impl RusToKModule for CommentsModule {
    fn slug(&self) -> &'static str {
        "comments"
    }

    fn name(&self) -> &'static str {
        "Comments"
    }

    fn description(&self) -> &'static str {
        "Generic comments domain for blog and other opt-in non-forum discussion surfaces"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Comments, Action::Create),
            Permission::new(Resource::Comments, Action::Read),
            Permission::new(Resource::Comments, Action::Update),
            Permission::new(Resource::Comments, Action::Delete),
            Permission::new(Resource::Comments, Action::List),
            Permission::new(Resource::Comments, Action::Moderate),
            Permission::new(Resource::Comments, Action::Manage),
        ]
    }
}

#[cfg(feature = "server")]
impl MigrationSource for CommentsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(feature = "server")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = CommentsModule;

        assert_eq!(module.slug(), "comments");
        assert_eq!(module.name(), "Comments");
        assert_eq!(
            module.description(),
            "Generic comments domain for blog and other opt-in non-forum discussion surfaces"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn module_permissions_cover_comment_lifecycle() {
        let module = CommentsModule;
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Moderate)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Manage)));
    }

    #[test]
    fn module_has_no_migrations_yet() {
        let module = CommentsModule;
        assert!(!module.migrations().is_empty());
    }
}
