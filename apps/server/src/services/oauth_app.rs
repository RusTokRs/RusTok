//! OAuth App Service — CRUD operations and credential management

use crate::auth::{self, AuthConfig};
use crate::models::oauth_apps::{self, ActiveModel as OAuthAppActiveModel, Entity as OAuthApps};
use crate::models::oauth_authorization_codes::{
    self, ActiveModel as OAuthCodeActiveModel, Entity as OAuthCodes,
};
use crate::models::oauth_consents::{
    self, ActiveModel as OAuthConsentActiveModel, Entity as OAuthConsents,
};
use crate::models::oauth_tokens::{self, Entity as OAuthTokens};
use chrono::Utc;
use loco_rs::{Error, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

/// Input for creating a new OAuth app
#[derive(Debug, Clone)]
pub struct CreateOAuthAppInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub app_type: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
}

/// Result of creating an OAuth app — includes the plaintext secret shown once
#[derive(Debug)]
pub struct CreateOAuthAppResult {
    pub app: oauth_apps::Model,
    pub client_secret: String,
}

/// Result of rotating an OAuth app's secret
#[derive(Debug)]
pub struct RotateSecretResult {
    pub app: oauth_apps::Model,
    pub client_secret: String,
}

pub struct OAuthAppService;

impl OAuthAppService {
    /// Create a new OAuth app with generated client_id and client_secret
    pub async fn create_app(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        input: CreateOAuthAppInput,
    ) -> Result<CreateOAuthAppResult> {
        let client_id = Uuid::new_v4();
        let client_secret_plain = generate_client_secret();
        let client_secret_hash = auth::hash_password(&client_secret_plain)?;

        let app = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(input.name),
            slug: Set(input.slug),
            description: Set(input.description),
            app_type: Set(input.app_type),
            icon_url: Set(None),
            client_id: Set(client_id),
            client_secret_hash: Set(Some(client_secret_hash)),
            redirect_uris: Set(serde_json::to_value(&input.redirect_uris)
                .map_err(|_| Error::InternalServerError)?),
            scopes: Set(
                serde_json::to_value(&input.scopes).map_err(|_| Error::InternalServerError)?
            ),
            grant_types: Set(
                serde_json::to_value(&input.grant_types).map_err(|_| Error::InternalServerError)?
            ),
            manifest_ref: Set(None),
            auto_created: Set(false),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(|e| Error::InternalServerError)?;

        Ok(CreateOAuthAppResult {
            app,
            client_secret: client_secret_plain,
        })
    }

    /// Find an active app by its public client_id
    pub async fn find_by_client_id(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<oauth_apps::Model>> {
        OAuthApps::find_active_by_client_id(db, client_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// List all apps for a tenant
    pub async fn list_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<oauth_apps::Model>> {
        OAuthApps::find_active_by_tenant(db, tenant_id)
            .await
            .map_err(|_| Error::InternalServerError)
    }

    /// Rotate client_secret — generates a new secret, returns plaintext once
    pub async fn rotate_secret(
        db: &DatabaseConnection,
        app_id: Uuid,
    ) -> Result<RotateSecretResult> {
        let app = OAuthApps::find_by_id(app_id)
            .one(db)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::NotFound)?;

        let new_secret = generate_client_secret();
        let new_hash = auth::hash_password(&new_secret)?;

        let mut active: OAuthAppActiveModel = app.into();
        active.client_secret_hash = Set(Some(new_hash));
        active.updated_at = Set(Utc::now().into());

        let updated = active
            .update(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        Ok(RotateSecretResult {
            app: updated,
            client_secret: new_secret,
        })
    }

    /// Revoke an app — deactivate and revoke all its tokens
    pub async fn revoke_app(db: &DatabaseConnection, app_id: Uuid) -> Result<oauth_apps::Model> {
        let app = OAuthApps::find_by_id(app_id)
            .one(db)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::NotFound)?;

        let now = Utc::now();

        // Deactivate the app
        let mut active: OAuthAppActiveModel = app.into();
        active.is_active = Set(false);
        active.revoked_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());

        let updated = active
            .update(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        // Revoke all active tokens for this app
        use sea_orm::sea_query::Expr;
        oauth_tokens::Entity::update_many()
            .col_expr(
                oauth_tokens::Column::RevokedAt,
                Expr::value(now.to_rfc3339()),
            )
            .filter(
                sea_orm::Condition::all()
                    .add(oauth_tokens::Column::AppId.eq(app_id))
                    .add(oauth_tokens::Column::RevokedAt.is_null()),
            )
            .exec(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        Ok(updated)
    }

    /// Verify a client_secret against the stored hash
    pub fn verify_client_secret(secret: &str, hash: &str) -> Result<bool> {
        auth::verify_password(secret, hash)
    }

    /// Issue a client_credentials access token for an app
    pub fn issue_client_credentials_token(
        app: &oauth_apps::Model,
        auth_config: &AuthConfig,
        requested_scopes: &[String],
    ) -> Result<(String, u64)> {
        // Validate requested scopes are within allowed scopes
        let allowed_scopes = app.scopes_list();
        let granted_scopes = if requested_scopes.is_empty() {
            allowed_scopes.clone()
        } else {
            requested_scopes
                .iter()
                .filter(|s| scope_matches(&allowed_scopes, s))
                .cloned()
                .collect()
        };

        // Service tokens get 1 hour TTL
        let expires_in = 3600u64;

        let token = auth::encode_oauth_access_token(
            auth_config,
            app.id,
            app.tenant_id,
            app.client_id,
            &granted_scopes,
            "client_credentials",
            expires_in,
        )?;

        Ok((token, expires_in))
    }

    /// Update last_used_at for an app
    pub async fn touch_last_used(db: &DatabaseConnection, app_id: Uuid) -> Result<()> {
        let app = OAuthApps::find_by_id(app_id)
            .one(db)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::NotFound)?;

        let mut active: OAuthAppActiveModel = app.into();
        active.last_used_at = Set(Some(Utc::now().into()));
        active
            .update(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        Ok(())
    }

    /// Generate and store an authorization code for an app + user
    pub async fn generate_authorization_code(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        redirect_uri: String,
        scopes: Vec<String>,
        code_challenge: String,
    ) -> Result<String> {
        // Generate random 43 character code
        use rand::Rng;
        let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen::<u8>()).collect();
        use base64::{engine::general_purpose, Engine as _};
        let code = general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes);

        // Hash it for DB storage
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        let code_hash = hex::encode(hasher.finalize());

        // Save to DB
        OAuthCodeActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(app_id),
            user_id: Set(user_id),
            tenant_id: Set(tenant_id),
            code_hash: Set(code_hash),
            redirect_uri: Set(redirect_uri),
            scopes: Set(serde_json::to_value(&scopes).map_err(|_| Error::InternalServerError)?),
            code_challenge: Set(code_challenge),
            code_challenge_method: Set("S256".to_string()),
            // Code valid for 10 minutes
            expires_at: Set((Utc::now() + chrono::Duration::minutes(10)).into()),
            used_at: Set(None),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(|_| Error::InternalServerError)?;

        Ok(code)
    }

    /// Exchange an authorization code for access/refresh tokens
    pub async fn exchange_authorization_code(
        db: &DatabaseConnection,
        app: &oauth_apps::Model,
        auth_config: &AuthConfig,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<(String, String, u64)> {
        // Hash the input code
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        let code_hash = hex::encode(hasher.finalize());

        // Find code in DB
        let auth_code = OAuthCodes::find_by_hash(db, &code_hash)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::Unauthorized("Invalid or expired code".into()))?;

        // Validations
        if !auth_code.is_active() {
            return Err(Error::Unauthorized(
                "Code is expired or already used".into(),
            ));
        }
        if auth_code.app_id != app.id {
            return Err(Error::Unauthorized(
                "Code belongs to a different app".into(),
            ));
        }
        if auth_code.redirect_uri != redirect_uri {
            return Err(Error::Unauthorized("Redirect URI mismatch".into()));
        }

        // Validate PKCE
        if !Self::verify_pkce(&auth_code.code_challenge, code_verifier) {
            return Err(Error::Unauthorized("PKCE code verifier is invalid".into()));
        }

        // Mark code as used
        let mut active: OAuthCodeActiveModel = auth_code.clone().into();
        active.used_at = Set(Some(Utc::now().into()));
        active
            .update(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        // Issue tokens
        let scopes = auth_code.scopes_list();
        Self::issue_authorization_token_pair(db, app, auth_config, auth_code.user_id, &scopes).await
    }

    /// Verify a PKCE code_verifier against the stored code_challenge
    /// code_challenge = BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))
    pub fn verify_pkce(code_challenge: &str, code_verifier: &str) -> bool {
        use base64::{engine::general_purpose, Engine as _};
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();

        let expected_challenge = general_purpose::URL_SAFE_NO_PAD.encode(hash);

        // Constant time comparison is safer for crypto
        use subtle::ConstantTimeEq;
        expected_challenge
            .as_bytes()
            .ct_eq(code_challenge.as_bytes())
            .into()
    }

    /// Issue an access token and a refresh token for authorization_code flow
    pub async fn issue_authorization_token_pair(
        db: &DatabaseConnection,
        app: &oauth_apps::Model,
        auth_config: &AuthConfig,
        user_id: Uuid,
        granted_scopes: &[String],
    ) -> Result<(String, String, u64)> {
        // User tokens get 15 min TTL (standard in our system)
        let expires_in = 900u64;

        // Note: For user context via OAuth, we need to embed the real user_id.
        // The token needs to look like a normal user token but with `client_id` set.
        let access_token = crate::auth::encode_access_token(
            auth_config,
            crate::auth::Claims {
                sub: user_id,
                tenant_id: app.tenant_id,
                role: rustok_core::UserRole::Customer, // Simplified for now, should look up real role
                session_id: Uuid::nil(), // No session ID for OAuth tokens explicitly mapped
                iss: "rustok".to_string(),
                aud: "rustok-api".to_string(),
                exp: (chrono::Utc::now().timestamp() as usize) + (expires_in as usize),
                iat: chrono::Utc::now().timestamp() as usize,
                client_id: Some(app.client_id),
                scopes: granted_scopes.to_vec(),
                grant_type: "authorization_code".to_string(),
            },
        )?;

        // Generate refresh token
        let refresh_token_plain = auth::generate_refresh_token();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(refresh_token_plain.as_bytes());
        let token_hash = hex::encode(hasher.finalize());

        crate::models::oauth_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(app.id),
            user_id: Set(Some(user_id)),
            tenant_id: Set(app.tenant_id),
            token_hash: Set(token_hash),
            grant_type: Set("authorization_code".to_string()),
            scopes: Set(
                serde_json::to_value(granted_scopes).map_err(|_| Error::InternalServerError)?
            ),
            // 30 days validity for refresh token
            expires_at: Set((chrono::Utc::now() + chrono::Duration::days(30)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(|_| Error::InternalServerError)?;

        Ok((access_token, refresh_token_plain, expires_in))
    }

    /// Refresh an access token using a refresh token
    pub async fn refresh_access_token(
        db: &DatabaseConnection,
        app: &oauth_apps::Model,
        auth_config: &AuthConfig,
        refresh_token: &str,
    ) -> Result<(String, String, u64)> {
        // Hash the input token
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(refresh_token.as_bytes());
        let token_hash = hex::encode(hasher.finalize());

        // Find token in DB
        let token_model = OAuthTokens::find_active_by_hash(db, &token_hash, app.id)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or_else(|| Error::Unauthorized("Invalid or expired refresh token".into()))?;

        // Extract required fields before doing anything
        let user_id = token_model
            .user_id
            .ok_or_else(|| Error::Unauthorized("Refresh token has no associated user".into()))?;
        let scopes = token_model.scopes_list();

        // Rotate the token (revoke the old one)
        let mut active: crate::models::oauth_tokens::ActiveModel = token_model.into();
        active.revoked_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        active
            .update(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        // Issue new token pair
        Self::issue_authorization_token_pair(db, app, auth_config, user_id, &scopes).await
    }

    /// Revoke a token by its hash (RFC 7009).
    /// Returns Ok(()) even if the token doesn't exist (per RFC 7009 spec).
    pub async fn revoke_token_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
        app_id: Uuid,
    ) -> Result<()> {
        let token = OAuthTokens::find_active_by_hash(db, token_hash, app_id)
            .await
            .map_err(|_| Error::InternalServerError)?;

        if let Some(t) = token {
            let mut active: oauth_tokens::ActiveModel = t.into();
            active.revoked_at = Set(Some(Utc::now().into()));
            active.updated_at = Set(Utc::now().into());
            active
                .update(db)
                .await
                .map_err(|_| Error::InternalServerError)?;
        }

        // Per RFC 7009: always succeed even if token not found
        Ok(())
    }

    /// Check if user has granted consent for the requested scopes
    pub async fn get_active_consent(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        requested_scopes: &[String],
    ) -> Result<bool> {
        let consent = OAuthConsents::find_active_consent(db, app_id, user_id)
            .await
            .map_err(|_| Error::InternalServerError)?;

        if let Some(c) = consent {
            let granted_scopes = c.scopes_list();
            // Check if all requested scopes are covered by granted scopes
            for req_scope in requested_scopes {
                if !scope_matches(&granted_scopes, req_scope) {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Grant or update user consent for an application
    pub async fn grant_consent(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
        scopes: Vec<String>,
    ) -> Result<()> {
        let existing = OAuthConsents::find_active_consent(db, app_id, user_id)
            .await
            .map_err(|_| Error::InternalServerError)?;

        if let Some(consent) = existing {
            // Merge scopes
            let mut current_scopes = consent.scopes_list();
            for new_scope in scopes {
                if !current_scopes.contains(&new_scope) {
                    // simplified array merge
                    current_scopes.push(new_scope);
                }
            }

            let mut active: OAuthConsentActiveModel = consent.into();
            active.scopes =
                Set(serde_json::to_value(&current_scopes)
                    .map_err(|_| Error::InternalServerError)?);
            active.granted_at = Set(Utc::now().into());
            active
                .update(db)
                .await
                .map_err(|_| Error::InternalServerError)?;
        } else {
            // Create new consent
            OAuthConsentActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(app_id),
                user_id: Set(user_id),
                tenant_id: Set(tenant_id),
                scopes: Set(serde_json::to_value(&scopes).map_err(|_| Error::InternalServerError)?),
                granted_at: Set(Utc::now().into()),
                revoked_at: Set(None),
            }
            .insert(db)
            .await
            .map_err(|_| Error::InternalServerError)?;
        }

        Ok(())
    }

    /// Revoke user consent for an application (and optionally tokens)
    pub async fn revoke_user_consent(
        db: &DatabaseConnection,
        app_id: Uuid,
        user_id: Uuid,
    ) -> Result<()> {
        let consent = OAuthConsents::find_active_consent(db, app_id, user_id)
            .await
            .map_err(|_| Error::InternalServerError)?;

        let now = Utc::now();

        if let Some(c) = consent {
            let mut active: OAuthConsentActiveModel = c.into();
            active.revoked_at = Set(Some(now.into()));
            active
                .update(db)
                .await
                .map_err(|_| Error::InternalServerError)?;
        }

        // Revoke all tokens for this user and app
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let _ = oauth_tokens::Entity::update_many()
            .col_expr(
                oauth_tokens::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now.to_rfc3339()),
            )
            .filter(
                sea_orm::Condition::all()
                    .add(oauth_tokens::Column::AppId.eq(app_id))
                    .add(oauth_tokens::Column::UserId.eq(user_id))
                    .add(oauth_tokens::Column::RevokedAt.is_null()),
            )
            .exec(db)
            .await
            .map_err(|_| Error::InternalServerError)?;

        Ok(())
    }
}

/// Sync OAuth app connections based on the modules manifest.
/// Called after a successful build to ensure apps match the deployment configuration.
///
/// Rules:
/// - embed_admin=true → upsert Embedded app "leptos-admin" (no secret, *:*)
/// - embed_storefront=true → upsert Embedded app "leptos-storefront" (no secret, *:*)
/// - Standalone storefronts → upsert FirstParty app per storefront entry
/// - Standalone admin → upsert FirstParty app for the admin stack
/// - Apps in DB that no longer match any manifest entry → soft-deactivate
pub async fn sync_app_connections(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    manifest: &crate::modules::manifest::ModulesManifest,
) -> Result<()> {
    let existing = OAuthAppService::list_by_tenant(db, tenant_id).await?;

    let mut active_slugs: Vec<String> = Vec::new();

    // 1. Embedded apps
    if manifest.build.server.embed_admin {
        upsert_embedded_app(db, tenant_id, "leptos-admin", &["*:*"]).await?;
        active_slugs.push("leptos-admin".to_string());
    }
    if manifest.build.server.embed_storefront {
        upsert_embedded_app(db, tenant_id, "leptos-storefront", &["*:*"]).await?;
        active_slugs.push("leptos-storefront".to_string());
    }

    // 2. Standalone storefronts → FirstParty apps
    for sf in &manifest.build.storefront {
        if !manifest.build.server.embed_storefront || sf.stack == "next" {
            upsert_first_party_app(
                db,
                tenant_id,
                &sf.id,
                &format!("{} storefront", sf.id),
                &["storefront:*"],
                &["authorization_code", "client_credentials"],
            )
            .await?;
            active_slugs.push(sf.id.clone());
        }
    }

    // 3. Standalone admin → FirstParty app
    if !manifest.build.server.embed_admin && !manifest.build.admin.stack.is_empty() {
        let slug = format!("{}-admin", manifest.build.admin.stack);
        upsert_first_party_app(
            db,
            tenant_id,
            &slug,
            &format!("{} Admin", manifest.build.admin.stack),
            &["admin:*"],
            &["authorization_code", "client_credentials"],
        )
        .await?;
        active_slugs.push(slug);
    }

    // 4. Deactivate orphaned auto-created apps
    for app in &existing {
        if app.auto_created && !active_slugs.contains(&app.slug) {
            let mut active: OAuthAppActiveModel = app.clone().into();
            active.is_active = Set(false);
            active.revoked_at = Set(Some(Utc::now().into()));
            active.updated_at = Set(Utc::now().into());
            let _ = active.update(db).await;
        }
    }

    Ok(())
}

/// Upsert an embedded app (no credentials needed)
async fn upsert_embedded_app(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
    scopes: &[&str],
) -> Result<()> {
    let existing = OAuthApps::find()
        .filter(
            sea_orm::Condition::all()
                .add(oauth_apps::Column::TenantId.eq(tenant_id))
                .add(oauth_apps::Column::Slug.eq(slug)),
        )
        .one(db)
        .await
        .map_err(|_| Error::InternalServerError)?;

    if let Some(app) = existing {
        // Re-activate if it was deactivated
        if !app.is_active {
            let mut active: OAuthAppActiveModel = app.into();
            active.is_active = Set(true);
            active.revoked_at = Set(None);
            active.updated_at = Set(Utc::now().into());
            active.update(db).await.map_err(|_| Error::InternalServerError)?;
        }
    } else {
        let scopes_vec: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
        OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(slug.to_string()),
            slug: Set(slug.to_string()),
            description: Set(Some("Auto-created embedded app".to_string())),
            app_type: Set("embedded".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(None), // Embedded — no secret
            redirect_uris: Set(serde_json::json!([])),
            scopes: Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?),
            grant_types: Set(serde_json::json!([])),
            manifest_ref: Set(Some(slug.to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(|_| Error::InternalServerError)?;
    }

    Ok(())
}

/// Upsert a first-party app (creates credentials on first creation)
async fn upsert_first_party_app(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
    scopes: &[&str],
    grant_types: &[&str],
) -> Result<()> {
    let existing = OAuthApps::find()
        .filter(
            sea_orm::Condition::all()
                .add(oauth_apps::Column::TenantId.eq(tenant_id))
                .add(oauth_apps::Column::Slug.eq(slug)),
        )
        .one(db)
        .await
        .map_err(|_| Error::InternalServerError)?;

    if let Some(app) = existing {
        // Re-activate if deactivated; update scopes/grant_types
        let scopes_vec: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
        let grant_types_vec: Vec<String> = grant_types.iter().map(|s| s.to_string()).collect();
        let mut active: OAuthAppActiveModel = app.into();
        active.is_active = Set(true);
        active.revoked_at = Set(None);
        active.scopes = Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?);
        active.grant_types = Set(serde_json::to_value(&grant_types_vec).map_err(|_| Error::InternalServerError)?);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await.map_err(|_| Error::InternalServerError)?;
    } else {
        let scopes_vec: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
        let grant_types_vec: Vec<String> = grant_types.iter().map(|s| s.to_string()).collect();
        let client_secret_plain = generate_client_secret();
        let client_secret_hash = auth::hash_password(&client_secret_plain)?;

        let app = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            name: Set(name.to_string()),
            slug: Set(slug.to_string()),
            description: Set(Some(format!("Auto-created for {slug}"))),
            app_type: Set("first_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(client_secret_hash)),
            redirect_uris: Set(serde_json::json!([])),
            scopes: Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?),
            grant_types: Set(serde_json::to_value(&grant_types_vec).map_err(|_| Error::InternalServerError)?),
            manifest_ref: Set(Some(slug.to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(|_| Error::InternalServerError)?;

        // Log the credentials (in production, these should go to a secure channel)
        tracing::info!(
            client_id = %app.client_id,
            slug = slug,
            "First-party OAuth app created. Client secret was generated (store securely)."
        );
    }

    Ok(())
}

/// Generate a client secret with `sk_live_` prefix
fn generate_client_secret() -> String {
    let token = auth::generate_refresh_token();
    format!("sk_live_{token}")
}

/// Check if a scope matches any of the allowed scopes (supports wildcards)
pub fn scope_matches(allowed: &[String], requested: &str) -> bool {
    for allowed_scope in allowed {
        if allowed_scope == "*:*" {
            return true;
        }
        if allowed_scope == requested {
            return true;
        }
        // Wildcard: "resource:*" matches "resource:read", "resource:write", etc.
        if let Some(prefix) = allowed_scope.strip_suffix(":*") {
            if let Some(req_prefix) = requested.split(':').next() {
                if prefix == req_prefix {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_matches_exact() {
        let allowed = vec!["catalog:read".to_string(), "orders:write".to_string()];
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(scope_matches(&allowed, "orders:write"));
        assert!(!scope_matches(&allowed, "admin:users"));
    }

    #[test]
    fn test_scope_matches_wildcard() {
        let allowed = vec!["storefront:*".to_string()];
        assert!(scope_matches(&allowed, "storefront:read"));
        assert!(scope_matches(&allowed, "storefront:write"));
        assert!(!scope_matches(&allowed, "admin:read"));
    }

    #[test]
    fn test_scope_matches_superadmin() {
        let allowed = vec!["*:*".to_string()];
        assert!(scope_matches(&allowed, "anything:here"));
        assert!(scope_matches(&allowed, "admin:users"));
    }

    #[test]
    fn test_scope_matches_empty() {
        let allowed: Vec<String> = vec![];
        assert!(!scope_matches(&allowed, "catalog:read"));
    }

    #[test]
    fn test_verify_pkce() {
        // Example from RFC 7636 Appendix B
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        assert!(OAuthAppService::verify_pkce(expected_challenge, verifier));
        assert!(!OAuthAppService::verify_pkce("wrong_challenge", verifier));
        assert!(!OAuthAppService::verify_pkce(
            expected_challenge,
            "wrong_verifier"
        ));
    }
}
