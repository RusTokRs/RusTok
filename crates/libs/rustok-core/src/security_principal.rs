use rustok_api::Permission;
use uuid::Uuid;

use crate::{SecurityContext, infer_user_role_from_permissions};

const CLIENT_CREDENTIALS_GRANT: &str = "client_credentials";

/// Build a domain `SecurityContext` from an already validated access-token
/// snapshot without collapsing service principals into user ownership.
///
/// Authentication is responsible for validating the grant/subject invariants.
/// This bridge only preserves the validated principal kind while carrying the
/// request-effective permission ceiling into module services.
pub fn security_context_from_access_token(
    subject_id: Uuid,
    grant_type: &str,
    permissions: &[Permission],
) -> SecurityContext {
    let role = infer_user_role_from_permissions(permissions);
    if grant_type == CLIENT_CREDENTIALS_GRANT {
        SecurityContext::service(role, permissions.iter().copied())
    } else {
        SecurityContext::from_permissions(role, Some(subject_id), permissions.iter().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::security_context_from_access_token;
    use crate::SecurityActorKind;
    use rustok_api::Permission;
    use uuid::Uuid;

    #[test]
    fn client_credentials_never_become_user_ownership() {
        let app_id = Uuid::new_v4();
        let context = security_context_from_access_token(
            app_id,
            "client_credentials",
            &[Permission::BLOG_POSTS_UPDATE],
        );

        assert_eq!(context.actor_kind, SecurityActorKind::Service);
        assert_eq!(context.user_id, None);
    }

    #[test]
    fn user_grants_preserve_the_authenticated_user_id() {
        let user_id = Uuid::new_v4();
        for grant in ["direct", "authorization_code"] {
            let context = security_context_from_access_token(
                user_id,
                grant,
                &[Permission::BLOG_POSTS_UPDATE],
            );
            assert_eq!(context.actor_kind, SecurityActorKind::User);
            assert_eq!(context.user_id, Some(user_id));
        }
    }
}
