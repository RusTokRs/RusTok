//! Host-composed read and rollback port for build transports.

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::{build::Model as Build, release::Model as Release};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildRollbackCommand {
    pub build_id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
}

impl BuildRollbackCommand {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.build_id.is_nil() || self.tenant_id.is_nil() || self.actor_id.is_nil() {
            anyhow::bail!("build rollback requires non-nil build, tenant, and actor identifiers");
        }
        Ok(())
    }
}

#[async_trait]
pub trait BuildControl: Send + Sync {
    async fn active_build(&self) -> anyhow::Result<Option<Build>>;

    async fn list_builds_page(&self, limit: u64, offset: u64) -> anyhow::Result<Vec<Build>>;

    async fn active_release(&self) -> anyhow::Result<Option<Release>>;

    async fn list_releases_page(&self, limit: u64, offset: u64) -> anyhow::Result<Vec<Release>>;

    async fn rollback_build(&self, command: BuildRollbackCommand) -> anyhow::Result<Build>;
}

#[derive(Clone)]
pub struct SharedBuildControl(pub Arc<dyn BuildControl>);

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::BuildRollbackCommand;

    fn command() -> BuildRollbackCommand {
        BuildRollbackCommand {
            build_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn rollback_command_requires_complete_actor_and_scope_identity() {
        assert!(command().validate().is_ok());

        let mut missing_build = command();
        missing_build.build_id = Uuid::nil();
        assert!(missing_build.validate().is_err());

        let mut missing_tenant = command();
        missing_tenant.tenant_id = Uuid::nil();
        assert!(missing_tenant.validate().is_err());

        let mut missing_actor = command();
        missing_actor.actor_id = Uuid::nil();
        assert!(missing_actor.validate().is_err());
    }
}
