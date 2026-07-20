/// Middleware to block REST auth endpoints for admin panel
///
/// Admin panel should use ONLY GraphQL for authentication.
/// Mixing REST and GraphQL is bad practice.
///
/// This middleware blocks requests to /api/auth/* endpoints
/// when they come from admin panel (detected via User-Agent or Referer).
use axum::{
    body::Body,
    http::{Request, Response, StatusCode, header},
    middleware::Next,
};

const BLOCKED_AUTH_PATHS: &[&str] = &[
    "/api/auth/login",
    "/api/auth/register",
    "/api/auth/logout",
    "/api/auth/refresh",
    "/api/auth/forgot-password",
    "/api/auth/reset-password",
];

const ADMIN_USER_AGENTS: &[&str] = &["RusToK-Admin", "RusToK-Leptos-Admin"];

/// Block REST auth endpoints for admin panel
pub async fn block_rest_auth_for_admin(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    let path = req.uri().path();

    // Check if path is a blocked REST auth endpoint
    if !BLOCKED_AUTH_PATHS.contains(&path) {
        // Not a REST auth endpoint, allow
        return Ok(next.run(req).await);
    }

    // Check if request is from admin panel
    let headers = req.headers();

    // Check User-Agent
    if let Some(user_agent) = headers.get(header::USER_AGENT) {
        if let Ok(ua_str) = user_agent.to_str() {
            if ADMIN_USER_AGENTS
                .iter()
                .any(|&admin_ua| ua_str.contains(admin_ua))
            {
                // Admin panel trying to use REST auth - block it!
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    // Check Referer (if coming from admin panel domain)
    if let Some(referer) = headers.get(header::REFERER) {
        if let Ok(referer_str) = referer.to_str() {
            // Check if referer contains admin port or path
            if referer_str.contains(":3001") || referer_str.contains("/admin") {
                // Admin panel trying to use REST auth - block it!
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    // Not from admin panel, allow
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_paths() {
        assert!(BLOCKED_AUTH_PATHS.contains(&"/api/auth/login"));
        assert!(BLOCKED_AUTH_PATHS.contains(&"/api/auth/register"));
        assert!(!BLOCKED_AUTH_PATHS.contains(&"/api/graphql"));
    }

    #[test]
    fn test_admin_user_agents() {
        assert!(ADMIN_USER_AGENTS.contains(&"RusToK-Admin"));
        assert!(ADMIN_USER_AGENTS.contains(&"RusToK-Leptos-Admin"));
    }
}
