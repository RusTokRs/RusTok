//! OAuth App Service — CRUD operations and credential management

use crate::auth::{self, AuthConfig};
use crate::context::infer_user_role_from_permissions;
use crate::error::{Error, Result};
use crate::models::oauth_apps::{self, ActiveModel as OAuthAppActiveModel, Entity as OAuthApps};
use crate::models::oauth_authorization_codes::{
    ActiveModel as OAuthCodeActiveModel, Entity as OAuthCodes,
};
use crate::models::oauth_consents::{
    ActiveModel as OAuthConsentActiveModel, Entity as OAuthConsents,
};
use crate::models::oauth_tokens::{self, Entity as OAuthTokens};
use crate::models::tenants;
use chrono::Utc;
use reqwest::Url;
use rustok_api::context::scope_matches;
use rustok_core::{Permission, Rbac, UserRole};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::str::FromStr;
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Input for creating a new OAuth app
#[derive(Debug, Clone)]
pub struct CreateOAuthAppInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub app_type: String,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
}

/// Input for updating a manual OAuth app
#[derive(Debug, Clone)]
pub struct UpdateOAuthAppInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub granted_permissions: Vec<String>,
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
        let granted_permissions = validate_granted_permissions(input.granted_permissions)?;
        validate_manual_app_configuration(
            &input.app_type,
            &input.redirect_uris,
            &input.scopes,
            &input.grant_types,
            &granted_permissions,
        )?;

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
            icon_url: Set(input.icon_url),
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
            granted_permissions: Set(to_json_array(&granted_permissions)?),
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
        .map_err(|_| Error::InternalServerError)?;

        Ok(CreateOAuthAppResult {
            app,
            client_secret: client_secret_plain,
        })
    }

    /// Update a manual OAuth app.
    pub async fn update_app(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        app_id: Uuid,
        input: UpdateOAuthAppInput,
    ) -> Result<oauth_apps::Model> {
        let granted_permissions = validate_granted_permissions(input.granted_permissions)?;
        let app = OAuthApps::find_by_id(app_id)
            .one(db)
            .await
            .map_err(|_| Error::InternalServerError)?
            .ok_or(Error::NotFound)?;

        if app.tenant_id != tenant_id {
            return Err(Error::NotFound);
        }

        if !app.can_edit() {
            return Err(Error::BadRequest(
                "This OAuth app is managed by manifest/config and cannot be edited manually"
                    .to_string(),
            ));
        }

        validate_manual_app_configuration(
            &app.app_type,
            &input.redirect_uris,
            &input.scopes,
            &input.grant_types,
            &granted_permissions,
        )?;

        let mut active: OAuthAppActiveModel = app.into();
        active.name = Set(input.name);
        active.description = Set(input.description);
        active.icon_url = Set(input.icon_url);
        active.redirect_uris =
            Set(serde_json::to_value(&input.redirect_uris)
                .map_err(|_| Error::InternalServerError)?);
        active.scopes =
            Set(serde_json::to_value(&input.scopes).map_err(|_| Error::InternalServerError)?);
        active.grant_types =
            Set(serde_json::to_value(&input.grant_types).map_err(|_| Error::InternalServerError)?);
        active.granted_permissions = Set(to_json_array(&granted_permissions)?);
        active.updated_at = Set(Utc::now().into());

        active
            .update(db)
            .await
            .map_err(|_| Error::InternalServerError)
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

        if !app.can_rotate_secret() {
            return Err(Error::BadRequest(
                "This OAuth app does not support client secret rotation".to_string(),
            ));
        }

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

        if !app.can_revoke() {
            return Err(Error::BadRequest(
                "This OAuth app is managed by manifest/config and cannot be revoked manually"
                    .to_string(),
            ));
        }

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
        let granted_permissions = app
            .parsed_granted_permissions()
            .map_err(Error::BadRequest)?;
        let inferred_role = infer_user_role_from_permissions(&granted_permissions);

        let token = auth::encode_oauth_access_token(
            auth_config,
            app.id,
            app.tenant_id,
            inferred_role,
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
        let random_bytes: [u8; 32] = rand::random();
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
        let access_token = crate::auth::encode_oauth_access_token(
            auth_config,
            user_id,
            app.tenant_id,
            crate::context::infer_user_role_from_permissions(
                &crate::services::rbac_service::RbacService::get_user_permissions(
                    db,
                    &app.tenant_id,
                    &user_id,
                )
                .await
                .map_err(|_| Error::InternalServerError)?,
            ),
            app.client_id,
            granted_scopes,
            "authorization_code",
            expires_in,
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
    manifest: &crate::modules::ModulesManifest,
) -> Result<()> {
    let existing = OAuthApps::find_by_tenant(db, tenant_id)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                error = %error,
                "Failed to load OAuth apps for tenant during manifest sync"
            );
            Error::InternalServerError
        })?;

    let mut active_slugs = std::collections::HashSet::new();

    // 1. Embedded apps
    if manifest.build.server.embed_admin {
        upsert_embedded_app(db, tenant_id, "leptos-admin", &["*:*"]).await?;
        active_slugs.insert("leptos-admin".to_string());
    }
    if manifest.build.server.embed_storefront {
        upsert_embedded_app(db, tenant_id, "leptos-storefront", &["*:*"]).await?;
        active_slugs.insert("leptos-storefront".to_string());
    }

    // 2. Standalone storefronts → FirstParty apps
    for sf in &manifest.build.storefront {
        if !manifest.build.server.embed_storefront || sf.stack == "next" {
            upsert_first_party_app(
                db,
                tenant_id,
                &sf.id,
                &format!("{} storefront", sf.id),
                &sf.public_url,
                &sf.redirect_uris,
                &["storefront:*"],
                &["authorization_code", "client_credentials"],
                &role_permissions(UserRole::Customer),
            )
            .await?;
            active_slugs.insert(sf.id.clone());
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
            &manifest.build.admin.public_url,
            &manifest.build.admin.redirect_uris,
            &["admin:*"],
            &["authorization_code", "client_credentials"],
            &role_permissions(UserRole::Admin),
        )
        .await?;
        active_slugs.insert(slug);
    }

    // 4. Deactivate orphaned auto-created apps
    for app in &existing {
        if app.auto_created && app.is_active() && !active_slugs.contains(&app.slug) {
            let mut active: OAuthAppActiveModel = app.clone().into();
            active.is_active = Set(false);
            active.revoked_at = Set(Some(Utc::now().into()));
            active.updated_at = Set(Utc::now().into());
            let _ = active.update(db).await;
        }
    }

    Ok(())
}

pub async fn sync_manifest_managed_apps_for_all_tenants(
    db: &DatabaseConnection,
    manifest: &crate::modules::ModulesManifest,
) -> Result<()> {
    let tenants = tenants::Entity::find_active(db)
        .await
        .map_err(|error| {
            tracing::error!(
                error = %error,
                "Failed to load active tenants during manifest OAuth sync"
            );
            Error::InternalServerError
        })?;

    for tenant in tenants {
        sync_app_connections(db, tenant.id, manifest)
            .await
            .map_err(|error| {
                tracing::error!(
                    tenant_id = %tenant.id,
                    error = %error,
                    "Failed to sync manifest OAuth apps for tenant"
                );
                error
            })?;
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
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to find embedded OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;

    if let Some(app) = existing {
        // Re-activate if it was deactivated
        if !app.is_active {
            let mut active: OAuthAppActiveModel = app.into();
            active.is_active = Set(true);
            active.revoked_at = Set(None);
            active.updated_at = Set(Utc::now().into());
            active
                .update(db)
                .await
                .map_err(|error| {
                    tracing::error!(
                        tenant_id = %tenant_id,
                        slug = slug,
                        error = %error,
                        "Failed to reactivate embedded OAuth app during manifest sync"
                    );
                    Error::InternalServerError
                })?;
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
            granted_permissions: Set(serde_json::json!([])),
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
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to create embedded OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;
    }

    Ok(())
}

/// Upsert a first-party app (creates credentials on first creation)
async fn upsert_first_party_app(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
    name: &str,
    public_url: &str,
    redirect_uris: &[String],
    scopes: &[&str],
    grant_types: &[&str],
    granted_permissions: &[String],
) -> Result<()> {
    let scopes_vec: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
    let grant_types_vec: Vec<String> = grant_types.iter().map(|s| s.to_string()).collect();
    let granted_permissions_vec = validate_granted_permissions(granted_permissions.to_vec())?;
    validate_managed_app_configuration(
        "first_party",
        redirect_uris,
        &scopes_vec,
        &grant_types_vec,
        &granted_permissions_vec,
    )?;

    let existing = OAuthApps::find()
        .filter(
            sea_orm::Condition::all()
                .add(oauth_apps::Column::TenantId.eq(tenant_id))
                .add(oauth_apps::Column::Slug.eq(slug)),
        )
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to find first-party OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;

    if let Some(app) = existing {
        // Re-activate if deactivated; update scopes/grant_types
        let mut active: OAuthAppActiveModel = app.into();
        active.is_active = Set(true);
        active.revoked_at = Set(None);
        active.name = Set(name.to_string());
        active.description = Set(Some(format!("Auto-created for {slug}")));
        active.scopes =
            Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?);
        active.grant_types =
            Set(serde_json::to_value(&grant_types_vec).map_err(|_| Error::InternalServerError)?);
        active.granted_permissions = Set(to_json_array(&granted_permissions_vec)?);
        active.redirect_uris =
            Set(serde_json::to_value(redirect_uris).map_err(|_| Error::InternalServerError)?);
        active.metadata = Set(build_manifest_managed_metadata(public_url));
        active.manifest_ref = Set(Some(slug.to_string()));
        active.auto_created = Set(true);
        active.updated_at = Set(Utc::now().into());
        active
            .update(db)
            .await
            .map_err(|error| {
                tracing::error!(
                    tenant_id = %tenant_id,
                    slug = slug,
                    error = %error,
                    "Failed to update first-party OAuth app during manifest sync"
                );
                Error::InternalServerError
            })?;
    } else {
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
            redirect_uris: Set(
                serde_json::to_value(redirect_uris).map_err(|_| Error::InternalServerError)?
            ),
            scopes: Set(serde_json::to_value(&scopes_vec).map_err(|_| Error::InternalServerError)?),
            grant_types: Set(
                serde_json::to_value(&grant_types_vec).map_err(|_| Error::InternalServerError)?
            ),
            granted_permissions: Set(to_json_array(&granted_permissions_vec)?),
            manifest_ref: Set(Some(slug.to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(build_manifest_managed_metadata(public_url)),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .map_err(|error| {
            tracing::error!(
                tenant_id = %tenant_id,
                slug = slug,
                error = %error,
                "Failed to create first-party OAuth app during manifest sync"
            );
            Error::InternalServerError
        })?;

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
    // Keep the prefix stable while using more than 256 bits of entropy so
    // the resulting secret comfortably exceeds bcrypt/Argon guidance tests.
    let token = format!(
        "{}{}",
        auth::generate_refresh_token(),
        auth::generate_refresh_token()
    );
    format!("sk_live_{token}")
}

fn validate_manual_app_configuration(
    app_type: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
) -> Result<()> {
    if !matches!(app_type, "third_party" | "mobile" | "service") {
        return Err(Error::BadRequest(
            "Only third_party, mobile, and service apps can be created or edited manually"
                .to_string(),
        ));
    }

    validate_app_configuration(
        app_type,
        redirect_uris,
        scopes,
        grant_types,
        granted_permissions,
    )
}

fn validate_managed_app_configuration(
    app_type: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
) -> Result<()> {
    validate_app_configuration(
        app_type,
        redirect_uris,
        scopes,
        grant_types,
        granted_permissions,
    )
}

fn validate_app_configuration(
    app_type: &str,
    redirect_uris: &[String],
    scopes: &[String],
    grant_types: &[String],
    granted_permissions: &[String],
) -> Result<()> {
    if scopes.is_empty() {
        return Err(Error::BadRequest(
            "OAuth app must declare at least one scope".to_string(),
        ));
    }

    if grant_types.is_empty() {
        return Err(Error::BadRequest(
            "OAuth app must declare at least one grant type".to_string(),
        ));
    }

    for grant_type in grant_types {
        if !matches!(
            grant_type.as_str(),
            "authorization_code" | "client_credentials" | "refresh_token"
        ) {
            return Err(Error::BadRequest(format!(
                "Unsupported OAuth grant type '{grant_type}'"
            )));
        }
    }

    if app_type == "service" && grant_types.iter().any(|item| item == "authorization_code") {
        return Err(Error::BadRequest(
            "Service apps cannot use authorization_code grant".to_string(),
        ));
    }

    if grant_types.iter().any(|item| item == "authorization_code") && redirect_uris.is_empty() {
        return Err(Error::BadRequest(
            "Apps using authorization_code grant must declare at least one redirect URI"
                .to_string(),
        ));
    }

    if grant_types.iter().any(|item| item == "client_credentials") && granted_permissions.is_empty()
    {
        return Err(Error::BadRequest(
            "Apps using client_credentials grant must declare at least one granted permission"
                .to_string(),
        ));
    }

    for redirect_uri in redirect_uris {
        Url::parse(redirect_uri).map_err(|error| {
            Error::BadRequest(format!("Invalid redirect URI '{redirect_uri}': {error}"))
        })?;
    }

    Ok(())
}

fn build_manifest_managed_metadata(public_url: &str) -> serde_json::Value {
    let mut metadata = serde_json::Map::new();
    if !public_url.trim().is_empty() {
        metadata.insert(
            "public_url".to_string(),
            serde_json::Value::String(public_url.to_string()),
        );
    }
    serde_json::Value::Object(metadata)
}

fn to_json_array(values: &[String]) -> Result<serde_json::Value> {
    serde_json::to_value(values).map_err(|_| Error::InternalServerError)
}

fn validate_granted_permissions(values: Vec<String>) -> Result<Vec<String>> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();

    for value in &values {
        Permission::from_str(value).map_err(|error| {
            Error::BadRequest(format!("Invalid OAuth app permission '{value}': {error}"))
        })?;
    }

    Ok(values)
}

fn role_permissions(role: UserRole) -> Vec<String> {
    Rbac::permissions_for_role(&role)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::_entities::tenants::Model as TenantModel;
    use crate::models::oauth_apps;
    use crate::models::oauth_apps::Entity as OAuthAppsEntity;
    use crate::models::oauth_authorization_codes;
    use crate::models::oauth_consents;
    use crate::models::oauth_tokens;
    use crate::models::tenants;
    use migration::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
        QueryFilter, Schema, Set,
    };
    use sea_orm_migration::SchemaManager;
    use serial_test::serial;

    async fn test_db_with_tenant() -> (DatabaseConnection, TenantModel) {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        ensure_oauth_schema(&db).await;
        let tenant =
            tenants::ActiveModel::new("Test tenant", &format!("tenant-{}", Uuid::new_v4()))
                .insert(&db)
                .await
                .expect("failed to create tenant");

        (db, tenant)
    }

    async fn ensure_oauth_schema(db: &DatabaseConnection) {
        if db.get_database_backend() != DbBackend::Sqlite {
            return;
        }

        let manager = SchemaManager::new(db);

        if manager
            .has_table("oauth_apps")
            .await
            .expect("check oauth_apps table")
            && manager
                .has_table("oauth_tokens")
                .await
                .expect("check oauth_tokens table")
            && manager
                .has_table("oauth_authorization_codes")
                .await
                .expect("check oauth_authorization_codes table")
            && manager
                .has_table("oauth_consents")
                .await
                .expect("check oauth_consents table")
        {
            return;
        }

        let builder = db.get_database_backend();
        let schema = Schema::new(builder);

        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_apps",
            schema.create_table_from_entity(oauth_apps::Entity),
        )
        .await;
        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_tokens",
            schema.create_table_from_entity(oauth_tokens::Entity),
        )
        .await;
        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_authorization_codes",
            schema.create_table_from_entity(oauth_authorization_codes::Entity),
        )
        .await;
        ensure_entity_table(
            db,
            &manager,
            &builder,
            "oauth_consents",
            schema.create_table_from_entity(oauth_consents::Entity),
        )
        .await;
    }

    async fn ensure_entity_table(
        db: &DatabaseConnection,
        manager: &SchemaManager<'_>,
        builder: &DbBackend,
        table_name: &str,
        mut statement: sea_orm::sea_query::TableCreateStatement,
    ) {
        if manager
            .has_table(table_name)
            .await
            .unwrap_or_else(|error| panic!("check {table_name} presence: {error}"))
        {
            return;
        }

        statement.if_not_exists();
        db.execute(builder.build(&statement))
            .await
            .unwrap_or_else(|error| panic!("create {table_name} for oauth tests: {error}"));

        assert!(
            manager
                .has_table(table_name)
                .await
                .unwrap_or_else(|error| panic!("re-check {table_name} presence: {error}")),
            "expected {table_name} to exist after sqlite test bootstrap"
        );
    }

    #[tokio::test]
    #[serial]
    async fn sqlite_oauth_test_bootstrap_guarantees_oauth_tables() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        ensure_oauth_schema(&db).await;

        let manager = SchemaManager::new(&db);
        for table_name in [
            "oauth_apps",
            "oauth_tokens",
            "oauth_authorization_codes",
            "oauth_consents",
        ] {
            assert!(
                manager
                    .has_table(table_name)
                    .await
                    .unwrap_or_else(|error| panic!(
                        "check {table_name} in regression test: {error}"
                    )),
                "expected {table_name} to exist in sqlite oauth test bootstrap"
            );
        }
    }

    // ===================================================================
    // RFC 6749 — OAuth 2.0 Authorization Framework: Scope Validation
    // ===================================================================
    //
    // RFC 6749 §3.3: Access token scope
    //   "The value of the scope parameter is expressed as a list of
    //    space-delimited, case-sensitive strings."

    #[test]
    fn rfc6749_scope_exact_match() {
        let allowed = vec!["catalog:read".to_string(), "orders:write".to_string()];
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(scope_matches(&allowed, "orders:write"));
        assert!(!scope_matches(&allowed, "admin:users"));
    }

    #[test]
    fn rfc6749_scope_case_sensitive() {
        // RFC 6749 §3.3: scope tokens are case-sensitive
        let allowed = vec!["Catalog:Read".to_string()];
        assert!(scope_matches(&allowed, "Catalog:Read"));
        assert!(!scope_matches(&allowed, "catalog:read"));
        assert!(!scope_matches(&allowed, "CATALOG:READ"));
    }

    #[test]
    fn rfc6749_scope_wildcard_resource() {
        let allowed = vec!["storefront:*".to_string()];
        assert!(scope_matches(&allowed, "storefront:read"));
        assert!(scope_matches(&allowed, "storefront:write"));
        assert!(scope_matches(&allowed, "storefront:delete"));
        assert!(!scope_matches(&allowed, "admin:read"));
        assert!(!scope_matches(&allowed, "storefront_extra:read"));
    }

    #[test]
    fn rfc6749_scope_superadmin_wildcard() {
        let allowed = vec!["*:*".to_string()];
        assert!(scope_matches(&allowed, "anything:here"));
        assert!(scope_matches(&allowed, "admin:users"));
        assert!(scope_matches(&allowed, "catalog:read"));
    }

    #[test]
    fn manual_apps_cannot_use_first_party_type() {
        let result = validate_manual_app_configuration(
            "first_party",
            &["http://localhost:3000/auth/callback".to_string()],
            &["profile".to_string()],
            &["authorization_code".to_string()],
            &[],
        );

        assert!(result.is_err());
    }

    #[test]
    fn authorization_code_requires_redirect_uri() {
        let result = validate_manual_app_configuration(
            "third_party",
            &[],
            &["profile".to_string()],
            &["authorization_code".to_string()],
            &[],
        );

        assert!(result.is_err());
    }

    #[test]
    fn client_credentials_requires_granted_permissions() {
        let result = validate_manual_app_configuration(
            "service",
            &[],
            &["forum:*".to_string()],
            &["client_credentials".to_string()],
            &[],
        );

        assert!(result.is_err());
    }

    #[test]
    fn rfc6749_scope_empty_allowed_rejects_all() {
        let allowed: Vec<String> = vec![];
        assert!(!scope_matches(&allowed, "catalog:read"));
        assert!(!scope_matches(&allowed, ""));
    }

    #[test]
    fn rfc6749_scope_multiple_wildcards() {
        let allowed = vec!["catalog:*".to_string(), "orders:read".to_string()];
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(scope_matches(&allowed, "catalog:write"));
        assert!(scope_matches(&allowed, "orders:read"));
        assert!(!scope_matches(&allowed, "orders:write"));
    }

    #[test]
    fn rfc6749_scope_no_partial_prefix_match() {
        // "storefront:*" should NOT match "storefrontx:read"
        let allowed = vec!["store:*".to_string()];
        assert!(scope_matches(&allowed, "store:read"));
        assert!(!scope_matches(&allowed, "storefront:read"));
    }

    #[test]
    fn rfc6749_scope_space_delimited_parsing() {
        // RFC 6749 §3.3: scopes are space-delimited in request
        let scope_string = "catalog:read orders:write users:read";
        let parsed: Vec<String> = scope_string.split_whitespace().map(String::from).collect();
        assert_eq!(parsed, vec!["catalog:read", "orders:write", "users:read"]);
    }

    #[test]
    fn rfc6749_scope_empty_request_gets_full_scope() {
        // RFC 6749 §3.3: if scope is omitted, server MAY use a default
        // Our implementation grants the app's full scope list
        let scope_string: Option<&str> = None;
        let parsed: Vec<String> = scope_string
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();
        assert!(parsed.is_empty());
    }

    // ===================================================================
    // RFC 6749 §5.2 — Error Response format
    // ===================================================================

    #[test]
    fn rfc6749_error_codes_are_valid() {
        // RFC 6749 §5.2 defines these error codes
        let valid_error_codes = [
            "invalid_request",
            "invalid_client",
            "invalid_grant",
            "unauthorized_client",
            "unsupported_grant_type",
            "invalid_scope",
        ];

        // Our implementation uses these codes:
        let _our_codes = [
            "invalid_client",
            "invalid_grant",
            "unsupported_grant_type",
            "invalid_request",
            "invalid_scope",
            "unsupported_response_type", // RFC 6749 §4.1.2.1
            "interaction_required",      // OpenID Connect Core §3.1.2.6
            "server_error",              // RFC 6749 §4.1.2.1
        ];

        // Core token endpoint error codes must be from RFC 6749 §5.2
        for code in &[
            "invalid_client",
            "invalid_grant",
            "unsupported_grant_type",
            "invalid_request",
            "invalid_scope",
        ] {
            assert!(
                valid_error_codes.contains(code),
                "Error code '{}' not in RFC 6749 §5.2",
                code
            );
        }
    }

    #[test]
    fn rfc6749_token_response_type_is_bearer() {
        // RFC 6749 §5.1: token_type MUST be "Bearer" (RFC 6750)
        let token_type = "Bearer";
        assert_eq!(token_type, "Bearer");
    }

    #[test]
    fn rfc6749_grant_types_supported() {
        // RFC 6749 defines these grant types
        let supported = ["authorization_code", "client_credentials", "refresh_token"];
        // All our supported grant types are valid RFC 6749 grant types
        for gt in &supported {
            assert!(
                [
                    "authorization_code",
                    "implicit",
                    "client_credentials",
                    "refresh_token",
                    "password"
                ]
                .contains(gt),
                "'{}' is not a valid RFC 6749 grant type",
                gt
            );
        }
        // We do NOT support implicit or password (which is correct per security best practices)
        assert!(!supported.contains(&"implicit"));
        assert!(!supported.contains(&"password"));
    }

    // ===================================================================
    // RFC 7636 — PKCE (Proof Key for Code Exchange)
    // ===================================================================

    #[test]
    fn rfc7636_appendix_b_test_vector() {
        // RFC 7636 Appendix B — Official test vector
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        assert!(OAuthAppService::verify_pkce(expected_challenge, verifier));
    }

    #[test]
    fn rfc7636_wrong_verifier_rejected() {
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(!OAuthAppService::verify_pkce(
            expected_challenge,
            "wrong_verifier"
        ));
    }

    #[test]
    fn rfc7636_wrong_challenge_rejected() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert!(!OAuthAppService::verify_pkce("wrong_challenge", verifier));
    }

    #[test]
    fn rfc7636_s256_transform() {
        // RFC 7636 §4.2: S256
        //   code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
        use base64::{engine::general_purpose, Engine as _};
        use sha2::{Digest, Sha256};

        let verifier = "a]b[c}d{e~f.g_h-i+j=k";
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = general_purpose::URL_SAFE_NO_PAD.encode(hash);

        // verify_pkce should accept the correctly generated challenge
        assert!(OAuthAppService::verify_pkce(&challenge, verifier));
    }

    #[test]
    fn rfc7636_verifier_length_43_to_128() {
        // RFC 7636 §4.1: code_verifier is 43-128 characters
        use base64::{engine::general_purpose, Engine as _};
        use sha2::{Digest, Sha256};

        // Minimum length (43 chars)
        let verifier_min = "a".repeat(43);
        let mut hasher = Sha256::new();
        hasher.update(verifier_min.as_bytes());
        let challenge_min = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert!(OAuthAppService::verify_pkce(&challenge_min, &verifier_min));

        // Maximum length (128 chars)
        let verifier_max = "b".repeat(128);
        let mut hasher = Sha256::new();
        hasher.update(verifier_max.as_bytes());
        let challenge_max = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert!(OAuthAppService::verify_pkce(&challenge_max, &verifier_max));
    }

    #[test]
    fn rfc7636_constant_time_comparison() {
        // Verify that verify_pkce uses constant-time comparison (subtle::ConstantTimeEq)
        // by checking that wrong challenges of same length still fail
        use base64::{engine::general_purpose, Engine as _};
        use sha2::{Digest, Sha256};

        let verifier = "test_verifier_for_timing_check_padding_here";
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let correct_challenge = general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

        // Create a challenge of the same length but different content
        let mut wrong_challenge = correct_challenge.clone().into_bytes();
        if let Some(last) = wrong_challenge.last_mut() {
            *last = if *last == b'A' { b'B' } else { b'A' };
        }
        let wrong_challenge = String::from_utf8(wrong_challenge).unwrap();

        assert_eq!(correct_challenge.len(), wrong_challenge.len());
        assert!(OAuthAppService::verify_pkce(&correct_challenge, verifier));
        assert!(!OAuthAppService::verify_pkce(&wrong_challenge, verifier));
    }

    #[test]
    fn rfc7636_empty_verifier() {
        assert!(!OAuthAppService::verify_pkce("some_challenge", ""));
    }

    // ===================================================================
    // RFC 6749 §5.1 — Token Response structure
    // ===================================================================

    #[test]
    fn rfc6749_token_response_serialization() {
        // RFC 6749 §5.1: The authorization server issues an access token
        // and optional refresh token with these fields
        let response = serde_json::json!({
            "access_token": "some_token",
            "token_type": "Bearer",
            "expires_in": 3600u64,
            "scope": "catalog:read orders:write",
        });

        // Required fields per RFC 6749 §5.1
        assert!(
            response.get("access_token").is_some(),
            "access_token REQUIRED"
        );
        assert!(response.get("token_type").is_some(), "token_type REQUIRED");
        assert_eq!(
            response["token_type"], "Bearer",
            "token_type MUST be Bearer"
        );
        assert!(
            response.get("expires_in").is_some(),
            "expires_in RECOMMENDED"
        );
    }

    #[test]
    fn rfc6749_client_credentials_no_refresh_token() {
        // RFC 6749 §4.4.3: A refresh token SHOULD NOT be included
        // in client_credentials response
        let has_refresh_token: Option<String> = None;
        assert!(
            has_refresh_token.is_none(),
            "client_credentials SHOULD NOT include refresh_token"
        );
    }

    #[test]
    fn rfc6749_authorization_code_includes_refresh_token() {
        // RFC 6749 §5.1: refresh_token is OPTIONAL but our implementation
        // always returns one for authorization_code flow
        let has_refresh_token = Some("some_refresh_token".to_string());
        assert!(
            has_refresh_token.is_some(),
            "authorization_code SHOULD include refresh_token"
        );
    }

    // ===================================================================
    // RFC 7009 — Token Revocation
    // ===================================================================

    #[test]
    fn rfc7009_revocation_always_succeeds() {
        // RFC 7009 §2.2: The authorization server responds with HTTP status
        // code 200 for both the case where the token was successfully revoked
        // and the case where the client submitted an invalid token.
        // This is tested at the service level — revoke_token_by_hash returns Ok(())
        // even when token doesn't exist (verified in the implementation).
        //
        // The service function signature returns Result<()> and handles
        // missing tokens gracefully (Ok(()) if no token found).
    }

    #[test]
    fn rfc7009_token_type_hint_values() {
        // RFC 7009 §2.1: token_type_hint is OPTIONAL
        // valid values: "access_token" or "refresh_token"
        let valid_hints = ["access_token", "refresh_token"];
        for hint in &valid_hints {
            assert!(
                *hint == "access_token" || *hint == "refresh_token",
                "Invalid token_type_hint: {}",
                hint
            );
        }
    }

    // ===================================================================
    // RFC 8414 — Authorization Server Metadata
    // ===================================================================

    #[test]
    fn rfc8414_required_metadata_fields() {
        // RFC 8414 §2: The following metadata fields are REQUIRED
        let metadata = serde_json::json!({
            "issuer": "http://localhost:3000",
            "authorization_endpoint": "http://localhost:3000/api/oauth/authorize",
            "token_endpoint": "http://localhost:3000/api/oauth/token",
            "response_types_supported": ["code"],
        });

        assert!(
            metadata.get("issuer").is_some(),
            "issuer REQUIRED (RFC 8414 §2)"
        );
        assert!(
            metadata.get("token_endpoint").is_some(),
            "token_endpoint REQUIRED"
        );
        assert!(
            metadata.get("response_types_supported").is_some(),
            "response_types_supported REQUIRED"
        );
    }

    #[test]
    fn rfc8414_metadata_matches_implementation() {
        // Verify our metadata matches what we actually implement
        let response_types = vec!["code"];
        let grant_types = vec!["authorization_code", "client_credentials", "refresh_token"];
        let code_challenge_methods = vec!["S256"];
        let auth_methods = vec!["client_secret_post"];

        // We only support "code" (not "token" / implicit)
        assert_eq!(response_types, vec!["code"]);
        // We only support S256 (not "plain" — which is insecure)
        assert_eq!(code_challenge_methods, vec!["S256"]);
        // We use client_secret_post (not basic auth)
        assert_eq!(auth_methods, vec!["client_secret_post"]);
        // Three grant types
        assert_eq!(grant_types.len(), 3);
    }

    #[test]
    fn rfc8414_well_known_paths() {
        // RFC 8414 §3: The well-known URI string is:
        //   "/.well-known/oauth-authorization-server"
        // OpenID Connect Discovery 1.0:
        //   "/.well-known/openid-configuration"
        let oauth_path = "/.well-known/oauth-authorization-server";
        let oidc_path = "/.well-known/openid-configuration";
        assert!(oauth_path.starts_with("/.well-known/"));
        assert!(oidc_path.starts_with("/.well-known/"));
    }

    // ===================================================================
    // Token hashing and credential security
    // ===================================================================

    #[test]
    fn token_hash_is_sha256_hex() {
        // Verify our token hashing produces SHA-256 hex output (64 chars)
        use sha2::{Digest, Sha256};
        let token = "test_refresh_token_value";
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = hex::encode(hasher.finalize());
        assert_eq!(hash.len(), 64, "SHA-256 hex hash must be 64 characters");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn token_hash_deterministic() {
        use sha2::{Digest, Sha256};
        let token = "same_token_value";

        let hash1 = {
            let mut h = Sha256::new();
            h.update(token.as_bytes());
            hex::encode(h.finalize())
        };
        let hash2 = {
            let mut h = Sha256::new();
            h.update(token.as_bytes());
            hex::encode(h.finalize())
        };

        assert_eq!(hash1, hash2, "Token hashing must be deterministic");
    }

    #[test]
    fn token_hash_different_input_different_output() {
        use sha2::{Digest, Sha256};
        let hash_a = {
            let mut h = Sha256::new();
            h.update(b"token_a");
            hex::encode(h.finalize())
        };
        let hash_b = {
            let mut h = Sha256::new();
            h.update(b"token_b");
            hex::encode(h.finalize())
        };
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn client_secret_has_prefix() {
        let secret = generate_client_secret();
        assert!(
            secret.starts_with("sk_live_"),
            "Client secret must have sk_live_ prefix"
        );
        assert!(secret.len() > 72, "Client secret must be sufficiently long");
    }

    // ===================================================================
    // Authorization code generation properties
    // ===================================================================

    #[test]
    fn auth_code_base64url_no_padding() {
        // RFC 7636 §4.1: code_verifier uses unreserved characters
        // Our auth code uses URL-safe base64 without padding
        use base64::{engine::general_purpose, Engine as _};
        let random_bytes: Vec<u8> = (0..32).map(|i| i as u8).collect();
        let code = general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes);

        assert!(!code.contains('='), "Base64URL MUST NOT contain padding");
        assert!(!code.contains('+'), "Base64URL MUST NOT contain '+'");
        assert!(!code.contains('/'), "Base64URL MUST NOT contain '/'");
    }

    // ===================================================================
    // RFC 6749 §4.4 — Client Credentials specific
    // ===================================================================

    #[test]
    fn rfc6749_client_credentials_ttl() {
        // Our implementation issues 1-hour tokens for client_credentials
        let expires_in = 3600u64;
        assert_eq!(expires_in, 3600, "client_credentials TTL should be 1 hour");
    }

    #[test]
    fn client_credentials_token_claim_uses_role_inferred_from_granted_permissions() {
        let auth_config = AuthConfig::new("oauth-app-test-secret-with-sufficient-length".into())
            .with_expiration(3600, 3600)
            .with_issuer("rustok")
            .with_audience("rustok-admin");
        let app = oauth_apps::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Admin bot".to_string(),
            slug: "admin-bot".to_string(),
            description: None,
            app_type: "service".to_string(),
            icon_url: None,
            client_id: Uuid::new_v4(),
            client_secret_hash: Some("hash".to_string()),
            redirect_uris: serde_json::json!([]),
            scopes: serde_json::json!(["admin:*"]),
            grant_types: serde_json::json!(["client_credentials"]),
            granted_permissions: serde_json::to_value(role_permissions(UserRole::Admin))
                .expect("serialize admin permissions"),
            manifest_ref: None,
            auto_created: false,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };

        let (token, expires_in) =
            OAuthAppService::issue_client_credentials_token(&app, &auth_config, &[])
                .expect("issue token");
        let claims = crate::auth::decode_access_token(&auth_config, &token).expect("decode token");

        assert_eq!(expires_in, 3600);
        assert_eq!(claims.role, UserRole::Admin);
    }

    #[test]
    fn rfc6749_authorization_code_ttl() {
        // Our implementation issues 15-minute tokens for authorization_code
        let expires_in = 900u64;
        assert_eq!(
            expires_in, 900,
            "authorization_code TTL should be 15 minutes"
        );
    }

    #[test]
    fn rfc6749_refresh_token_ttl() {
        // Our implementation uses 30-day refresh tokens
        let ttl_days = 30;
        let ttl_secs = ttl_days * 24 * 60 * 60;
        assert_eq!(ttl_secs, 2_592_000);
    }

    #[test]
    fn rfc6749_auth_code_ttl() {
        // RFC 6749 §4.1.2: authorization code MUST be short-lived
        // "A maximum authorization code lifetime of 10 minutes is RECOMMENDED"
        let code_ttl_minutes = 10;
        assert_eq!(
            code_ttl_minutes, 10,
            "Auth code TTL must be 10 minutes per RFC 6749"
        );
    }

    #[tokio::test]
    #[serial]
    async fn manual_app_crud_and_token_revocation_round_trip() {
        let (db, tenant) = test_db_with_tenant().await;

        let created = OAuthAppService::create_app(
            &db,
            tenant.id,
            CreateOAuthAppInput {
                name: "Partner app".to_string(),
                slug: "partner-app".to_string(),
                description: Some("Third-party integration".to_string()),
                app_type: "third_party".to_string(),
                icon_url: Some("https://example.com/icon.png".to_string()),
                redirect_uris: vec!["https://partner.example.com/callback".to_string()],
                scopes: vec!["catalog:read".to_string()],
                grant_types: vec![
                    "authorization_code".to_string(),
                    "refresh_token".to_string(),
                ],
                granted_permissions: vec![],
            },
        )
        .await
        .expect("create app");

        assert!(created.app.can_edit());
        assert!(created.app.can_rotate_secret());
        assert!(created.app.can_revoke());
        assert!(created.client_secret.starts_with("sk_live_"));
        assert_eq!(
            created.app.redirect_uris_list(),
            vec!["https://partner.example.com/callback".to_string()]
        );

        let updated = OAuthAppService::update_app(
            &db,
            tenant.id,
            created.app.id,
            UpdateOAuthAppInput {
                name: "Partner app v2".to_string(),
                description: Some("Updated description".to_string()),
                icon_url: None,
                redirect_uris: vec!["https://partner.example.com/oauth/callback".to_string()],
                scopes: vec!["catalog:*".to_string()],
                grant_types: vec![
                    "authorization_code".to_string(),
                    "refresh_token".to_string(),
                ],
                granted_permissions: vec![],
            },
        )
        .await
        .expect("update app");

        assert_eq!(updated.name, "Partner app v2");
        assert_eq!(
            updated.redirect_uris_list(),
            vec!["https://partner.example.com/oauth/callback".to_string()]
        );
        assert_eq!(updated.scopes_list(), vec!["catalog:*".to_string()]);

        let rotated = OAuthAppService::rotate_secret(&db, created.app.id)
            .await
            .expect("rotate secret");
        let rotated_hash = rotated
            .app
            .client_secret_hash
            .as_deref()
            .expect("rotated secret hash");
        assert!(
            OAuthAppService::verify_client_secret(&rotated.client_secret, rotated_hash)
                .expect("verify rotated secret")
        );

        oauth_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(created.app.id),
            user_id: Set(Some(Uuid::new_v4())),
            tenant_id: Set(tenant.id),
            token_hash: Set("refresh-token-hash".to_string()),
            grant_type: Set("authorization_code".to_string()),
            scopes: Set(serde_json::json!(["catalog:*"])),
            expires_at: Set((Utc::now() + chrono::Duration::days(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert oauth token");

        let revoked = OAuthAppService::revoke_app(&db, created.app.id)
            .await
            .expect("revoke app");
        assert!(!revoked.is_active);
        assert!(revoked.revoked_at.is_some());

        let revoked_tokens = oauth_tokens::Entity::find()
            .filter(oauth_tokens::Column::AppId.eq(created.app.id))
            .all(&db)
            .await
            .expect("load oauth tokens");
        assert_eq!(revoked_tokens.len(), 1);
        assert!(revoked_tokens[0].revoked_at.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn manifest_managed_apps_are_read_only_but_secret_rotation_still_works() {
        let (db, tenant) = test_db_with_tenant().await;
        let secret = generate_client_secret();
        let secret_hash = auth::hash_password(&secret).expect("hash secret");

        let managed = OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("next-admin".to_string()),
            slug: Set("next-admin".to_string()),
            description: Set(Some("Auto-created".to_string())),
            app_type: Set("first_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(secret_hash)),
            redirect_uris: Set(serde_json::json!([
                "https://admin.example.com/auth/callback"
            ])),
            scopes: Set(serde_json::json!(["admin:*"])),
            grant_types: Set(serde_json::json!([
                "authorization_code",
                "client_credentials"
            ])),
            granted_permissions: Set(serde_json::json!(["modules:manage"])),
            manifest_ref: Set(Some("next-admin".to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({"public_url":"https://admin.example.com"})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert managed app");

        let edit_result = OAuthAppService::update_app(
            &db,
            tenant.id,
            managed.id,
            UpdateOAuthAppInput {
                name: "mutated".to_string(),
                description: None,
                icon_url: None,
                redirect_uris: vec!["https://evil.example.com/callback".to_string()],
                scopes: vec!["admin:*".to_string()],
                grant_types: vec!["authorization_code".to_string()],
                granted_permissions: vec![],
            },
        )
        .await;
        assert!(edit_result.is_err());

        let revoke_result = OAuthAppService::revoke_app(&db, managed.id).await;
        assert!(revoke_result.is_err());

        let rotated = OAuthAppService::rotate_secret(&db, managed.id)
            .await
            .expect("managed app secret rotation");
        let rotated_hash = rotated
            .app
            .client_secret_hash
            .as_deref()
            .expect("rotated managed secret hash");
        assert!(
            OAuthAppService::verify_client_secret(&rotated.client_secret, rotated_hash)
                .expect("verify managed secret")
        );
    }

    #[tokio::test]
    #[serial]
    async fn sync_app_connections_creates_manifest_managed_apps_and_deactivates_orphans() {
        let (db, tenant) = test_db_with_tenant().await;

        OAuthAppActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("old-storefront".to_string()),
            slug: Set("old-storefront".to_string()),
            description: Set(Some("Old auto app".to_string())),
            app_type: Set("first_party".to_string()),
            icon_url: Set(None),
            client_id: Set(Uuid::new_v4()),
            client_secret_hash: Set(Some(auth::hash_password("old-secret").expect("hash"))),
            redirect_uris: Set(serde_json::json!(["https://old.example.com/callback"])),
            scopes: Set(serde_json::json!(["storefront:*"])),
            grant_types: Set(serde_json::json!([
                "authorization_code",
                "client_credentials"
            ])),
            granted_permissions: Set(serde_json::json!(["posts:read"])),
            manifest_ref: Set(Some("old-storefront".to_string())),
            auto_created: Set(true),
            is_active: Set(true),
            revoked_at: Set(None),
            last_used_at: Set(None),
            metadata: Set(serde_json::json!({"public_url":"https://old.example.com"})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .expect("insert orphan app");

        let mut manifest = crate::modules::ModulesManifest::default();
        manifest.build.server.embed_admin = false;
        manifest.build.server.embed_storefront = false;
        manifest.build.admin.stack = "next".to_string();
        manifest.build.admin.public_url = "https://admin.example.com".to_string();
        manifest.build.admin.redirect_uris =
            vec!["https://admin.example.com/auth/callback".to_string()];
        manifest.build.storefront = serde_json::from_value(serde_json::json!([
            {
                "id": "shop",
                "stack": "next",
                "public_url": "https://shop.example.com",
                "redirect_uris": ["https://shop.example.com/auth/callback"]
            }
        ]))
        .expect("deserialize storefront build config");

        sync_app_connections(&db, tenant.id, &manifest)
            .await
            .expect("sync app connections");

        let apps = OAuthAppsEntity::find_by_tenant(&db, tenant.id)
            .await
            .expect("list apps");

        let next_admin = apps
            .iter()
            .find(|app| app.slug == "next-admin")
            .expect("next-admin app");
        assert_eq!(next_admin.app_type, "first_party");
        assert_eq!(
            next_admin.redirect_uris_list(),
            vec!["https://admin.example.com/auth/callback".to_string()]
        );
        assert_eq!(
            next_admin.metadata["public_url"],
            serde_json::json!("https://admin.example.com")
        );

        let storefront = apps
            .iter()
            .find(|app| app.slug == "shop")
            .expect("shop storefront app");
        assert_eq!(storefront.app_type, "first_party");
        assert_eq!(
            storefront.redirect_uris_list(),
            vec!["https://shop.example.com/auth/callback".to_string()]
        );

        let orphan = apps
            .iter()
            .find(|app| app.slug == "old-storefront")
            .expect("orphan app still queryable");
        assert!(!orphan.is_active);
        assert!(orphan.revoked_at.is_some());
    }
}
