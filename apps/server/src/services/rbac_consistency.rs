use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;
pub use rustok_rbac::RbacConsistencyStats;

pub async fn load_rbac_consistency_stats(
    ctx: &ServerRuntimeContext,
) -> Result<RbacConsistencyStats> {
    rustok_rbac::load_consistency_stats(ctx.db())
        .await
        .map_err(|error| Error::Message(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::RbacConsistencyStats;

    #[test]
    fn stats_default_is_zeroed() {
        let stats = RbacConsistencyStats::default();
        assert_eq!(stats.users_without_roles_total, 0);
        assert_eq!(stats.orphan_user_roles_total, 0);
        assert_eq!(stats.orphan_role_permissions_total, 0);
    }
}
