# Security Audit Guide

Полное руководство по безопасности системы revision history.

## Содержание

1. [Threat Model](#threat-model)
2. [Authentication](#authentication)
3. [Authorization](#authorization)
4. [Data Protection](#data-protection)
5. [Input Validation](#input-validation)
6. [SQL Injection Prevention](#sql-injection-prevention)
7. [XSS Prevention](#xss-prevention)
8. [Rate Limiting](#rate-limiting)
9. [Audit Logging](#audit-logging)
10. [Compliance](#compliance)
11. [Security Checklist](#security-checklist)

## Threat Model

### Assets

**Данные, которые нужно защитить:**

1. **Revision History**
   - История изменений контента
   - Дельты изменений
   - Метаданные (кто, когда, почему)

2. **User Data**
   - Идентификаторы пользователей
   - Информация о действиях пользователей
   - IP адреса

3. **System Data**
   - Конфигурация системы
   - Credentials
   - API keys

### Threats

**Потенциальные угрозы:**

1. **Unauthorized Access**
   - Несанкционированный доступ к истории изменений
   - Просмотр чужих ревизий
   - Восстановление без прав

2. **Data Tampering**
   - Изменение истории ревизий
   - Подделка дельт
   - Удаление ревизий

3. **Data Leakage**
   - Утечка чувствительных данных через ревизии
   - Экспорт истории без авторизации
   - SQL injection для получения данных

4. **Denial of Service**
   - Создание большого количества ревизий
   - Запросы больших объемов данных
   - Resource exhaustion

5. **Privilege Escalation**
   - Доступ к ревизиям других tenants
   - Обход retention policy
   - Изменение чужих named versions

### Attack Vectors

**Возможные векторы атак:**

1. **API Endpoints**
   - GraphQL queries/mutations
   - REST endpoints
   - WebSocket connections

2. **Database**
   - SQL injection
   - Direct database access
   - Backup files

3. **Application**
   - Insecure deserialization
   - Memory corruption
   - Race conditions

4. **Infrastructure**
   - Network sniffing
   - Man-in-the-middle attacks
   - Compromised servers

## Authentication

### 1. JWT Authentication

**Implementation:**

```rust
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // User ID
    pub tenant_id: String,  // Tenant ID
    pub roles: Vec<String>, // User roles
    pub exp: usize,         // Expiration time
    pub iat: usize,         // Issued at
}

pub fn create_token(user: &User, secret: &str) -> Result<String, RevisionError> {
    let claims = Claims {
        sub: user.id.clone(),
        tenant_id: user.tenant_id.clone(),
        roles: user.roles.clone(),
        exp: (Utc::now() + Duration::hours(24)).timestamp() as usize,
        iat: Utc::now().timestamp() as usize,
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .map_err(|e| RevisionError::AuthenticationError(e.to_string()))
}

pub fn validate_token(token: &str, secret: &str) -> Result<Claims, RevisionError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| RevisionError::AuthenticationError(e.to_string()))
}
```

**Middleware:**

```rust
use axum::{
    extract::Extension,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn auth_middleware(
    Extension(state): Extension<AppState>,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Result<Response, StatusCode> {
    let headers = req.headers();
    
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let claims = validate_token(token, &state.jwt_secret)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    req.extensions_mut().insert(claims);
    
    Ok(next.run(req).await)
}
```

### 2. API Key Authentication

**For service-to-service communication:**

```rust
pub async fn api_key_middleware(
    Extension(state): Extension<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Result<Response, StatusCode> {
    let headers = req.headers();
    
    let api_key = headers
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Validate API key
    let service = state.api_keys.validate(api_key).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Check if API key has required permissions
    if !service.has_permission("revisions:read") {
        return Err(StatusCode::FORBIDDEN);
    }
    
    Ok(next.run(req).await)
}
```

### 3. OAuth 2.0

**Integration with OAuth providers:**

```rust
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
};

pub fn create_oauth_client() -> BasicClient {
    BasicClient::new(
        ClientId::new(env::var("OAUTH_CLIENT_ID").unwrap()),
        Some(ClientSecret::new(env::var("OAUTH_CLIENT_SECRET").unwrap())),
        AuthUrl::new(env::var("OAUTH_AUTH_URL").unwrap()).unwrap(),
        Some(TokenUrl::new(env::var("OAUTH_TOKEN_URL").unwrap()).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(env::var("OAUTH_REDIRECT_URL").unwrap()).unwrap())
}
```

## Authorization

### 1. Role-Based Access Control (RBAC)

**Define roles:**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

impl Role {
    pub fn can_create_revision(&self) -> bool {
        matches!(self, Role::Admin | Role::Editor)
    }
    
    pub fn can_restore_revision(&self) -> bool {
        matches!(self, Role::Admin | Role::Editor)
    }
    
    pub fn can_delete_revision(&self) -> bool {
        matches!(self, Role::Admin)
    }
    
    pub fn can_view_revision(&self) -> bool {
        true // All roles can view
    }
}
```

**Authorization middleware:**

```rust
pub async fn require_role(
    Extension(claims): Extension<Claims>,
    required_role: Role,
) -> Result<(), StatusCode> {
    let user_roles: Vec<Role> = claims.roles.iter()
        .filter_map(|r| r.parse().ok())
        .collect();
    
    if !user_roles.contains(&required_role) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    Ok(())
}
```

### 2. Resource-Based Authorization

**Check ownership:**

```rust
pub async fn check_revision_access(
    claims: &Claims,
    revision: &Revision,
) -> Result<(), RevisionError> {
    // Check tenant isolation
    if claims.tenant_id != revision.tenant_id {
        return Err(RevisionError::Forbidden);
    }
    
    // Check content ownership (if applicable)
    let content = get_content(&revision.content_type, &revision.content_id).await?;
    if content.owner_id != claims.sub && !claims.roles.contains(&"admin".to_string()) {
        return Err(RevisionError::Forbidden);
    }
    
    Ok(())
}
```

### 3. Attribute-Based Access Control (ABAC)

**Policy-based authorization:**

```rust
pub struct AccessPolicy {
    pub allow_read: bool,
    pub allow_write: bool,
    pub allow_delete: bool,
}

pub fn evaluate_policy(
    claims: &Claims,
    revision: &Revision,
    context: &Context,
) -> AccessPolicy {
    let is_owner = revision.created_by == claims.sub;
    let is_admin = claims.roles.contains(&"admin".to_string());
    let is_same_tenant = revision.tenant_id == claims.tenant_id;
    
    AccessPolicy {
        allow_read: is_same_tenant,
        allow_write: is_same_tenant && (is_owner || is_admin),
        allow_delete: is_same_tenant && is_admin,
    }
}
```

## Data Protection

### 1. Encryption at Rest

**Encrypt sensitive fields:**

```rust
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};

pub fn encrypt_delta(delta: &serde_json::Value, key: &[u8]) -> Result<Vec<u8>, RevisionError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| RevisionError::EncryptionError(e.to_string()))?;
    
    let nonce = Aes256Gcm::generate_nonce(&mut rand::thread_rng());
    let plaintext = serde_json::to_vec(delta)?;
    
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .map_err(|e| RevisionError::EncryptionError(e.to_string()))?;
    
    let mut result = nonce.to_vec();
    result.extend(ciphertext);
    
    Ok(result)
}

pub fn decrypt_delta(encrypted: &[u8], key: &[u8]) -> Result<serde_json::Value, RevisionError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| RevisionError::EncryptionError(e.to_string()))?;
    
    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext = &encrypted[12..];
    
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| RevisionError::EncryptionError(e.to_string()))?;
    
    Ok(serde_json::from_slice(&plaintext)?)
}
```

### 2. Encryption in Transit

**TLS configuration:**

```rust
use axum_server::tls_rustls::RustlsConfig;

pub async fn start_https_server() {
    let config = RustlsConfig::from_pem_file(
        "certs/cert.pem",
        "certs/key.pem",
    )
    .await
    .unwrap();
    
    axum_server::bind_rustls("0.0.0.0:443".parse().unwrap(), config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

**TLS settings:**

```rust
use rustls::{ServerConfig, Certificate, PrivateKey};

pub fn create_tls_config() -> ServerConfig {
    let certs = load_certs("certs/cert.pem");
    let key = load_private_key("certs/key.pem");
    
    ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap()
}
```

### 3. Data Masking

**Mask sensitive data in logs:**

```rust
pub fn mask_sensitive_data(delta: &serde_json::Value) -> serde_json::Value {
    let sensitive_fields = ["password", "ssn", "credit_card", "email"];
    
    let mut masked = delta.clone();
    
    if let Some(obj) = masked.as_object_mut() {
        for field in sensitive_fields {
            if obj.contains_key(field) {
                obj.insert(field.to_string(), json!("***MASKED***"));
            }
        }
    }
    
    masked
}
```

### 4. Secure Key Management

**Use environment variables:**

```rust
use std::env;

pub struct SecurityConfig {
    pub jwt_secret: String,
    pub encryption_key: Vec<u8>,
    pub api_keys: Vec<String>,
}

impl SecurityConfig {
    pub fn from_env() -> Result<Self, RevisionError> {
        Ok(Self {
            jwt_secret: env::var("JWT_SECRET")
                .map_err(|_| RevisionError::ConfigError("JWT_SECRET not set".to_string()))?,
            encryption_key: env::var("ENCRYPTION_KEY")
                .map_err(|_| RevisionError::ConfigError("ENCRYPTION_KEY not set".to_string()))?
                .into_bytes(),
            api_keys: env::var("API_KEYS")
                .unwrap_or_default()
                .split(',')
                .map(String::from)
                .collect(),
        })
    }
}
```

**Use HashiCorp Vault:**

```rust
use vaultrs::{client::VaultClient, kv2};

pub async fn get_secret_from_vault(
    client: &VaultClient,
    path: &str,
) -> Result<String, RevisionError> {
    let secret: serde_json::Value = kv2::read(client, "secret", path, None)
        .await
        .map_err(|e| RevisionError::VaultError(e.to_string()))?;
    
    secret["data"]["value"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| RevisionError::VaultError("Secret not found".to_string()))
}
```

## Input Validation

### 1. Validate GraphQL Inputs

```rust
use validator::Validate;

#[derive(Validate)]
pub struct CreateRevisionInput {
    #[validate(length(min = 1, max = 255))]
    pub content_id: String,
    
    #[validate(length(min = 1, max = 255))]
    pub content_type: String,
    
    #[validate(length(min = 2, max = 10))]
    pub locale: Option<String>,
    
    #[validate(length(max = 1000))]
    pub change_summary: Option<String>,
}

pub async fn create_revision(
    input: CreateRevisionInput,
    claims: Claims,
) -> Result<Revision, RevisionError> {
    // Validate input
    input.validate()
        .map_err(|e| RevisionError::ValidationError(e.to_string()))?;
    
    // Sanitize input
    let content_id = sanitize_string(&input.content_id);
    let content_type = sanitize_string(&input.content_type);
    
    // Process revision
    // ...
}
```

### 2. Sanitize User Input

```rust
pub fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(255)
        .collect()
}

pub fn sanitize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            serde_json::Value::String(sanitize_string(s))
        }
        serde_json::Value::Object(obj) => {
            let mut sanitized = serde_json::Map::new();
            for (key, val) in obj {
                let sanitized_key = sanitize_string(key);
                let sanitized_val = sanitize_json(val);
                sanitized.insert(sanitized_key, sanitized_val);
            }
            serde_json::Value::Object(sanitized)
        }
        _ => value.clone(),
    }
}
```

### 3. Validate JSON Schema

```rust
use jsonschema::JSONSchema;

pub fn validate_content_schema(
    content: &serde_json::Value,
    content_type: &str,
) -> Result<(), RevisionError> {
    let schema = get_schema_for_type(content_type)?;
    let compiled = JSONSchema::compile(&schema)
        .map_err(|e| RevisionError::SchemaError(e.to_string()))?;
    
    compiled.validate(content)
        .map_err(|errors| {
            let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
            RevisionError::ValidationError(error_messages.join(", "))
        })?;
    
    Ok(())
}
```

## SQL Injection Prevention

### 1. Use Parameterized Queries

```rust
// ✅ Хорошо: parameterized query
pub async fn get_revision(
    pool: &PgPool,
    id: Uuid,
) -> Result<Revision, RevisionError> {
    let revision = sqlx::query_as::<_, Revision>(
        "SELECT * FROM content_revisions WHERE id = $1"
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    
    Ok(revision)
}

// ❌ Плохо: string interpolation (SQL injection vulnerability!)
pub async fn get_revision_unsafe(
    pool: &PgPool,
    id: &str,
) -> Result<Revision, RevisionError> {
    let query = format!("SELECT * FROM content_revisions WHERE id = '{}'", id);
    let revision = sqlx::query_as::<_, Revision>(&query)
        .fetch_one(pool)
        .await?;
    
    Ok(revision)
}
```

### 2. Use ORM (SeaORM)

```rust
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

pub async fn get_revision_orm(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<Revision, RevisionError> {
    let revision = ContentRevisions::find()
        .filter(content_revisions::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or(RevisionError::NotFound)?;
    
    Ok(revision)
}
```

### 3. Escape Special Characters

```rust
pub fn escape_sql_string(input: &str) -> String {
    input
        .replace("'", "''")
        .replace("\\", "\\\\")
        .replace("\0", "\\0")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\x1a", "\\Z")
}
```

## XSS Prevention

### 1. Escape Output

```rust
use html_escape::encode_text;

pub fn render_revision_html(revision: &Revision) -> String {
    format!(
        "<div class='revision'>
            <h3>{}</h3>
            <p>{}</p>
        </div>",
        encode_text(&revision.change_summary.unwrap_or_default()),
        encode_text(&format!("Revision #{}", revision.revision_number))
    )
}
```

### 2. Content Security Policy

```rust
use axum::http::HeaderMap;

pub fn add_csp_headers(headers: &mut HeaderMap) {
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';"
            .parse()
            .unwrap(),
    );
    
    headers.insert(
        "X-Content-Type-Options",
        "nosniff".parse().unwrap(),
    );
    
    headers.insert(
        "X-Frame-Options",
        "DENY".parse().unwrap(),
    );
    
    headers.insert(
        "X-XSS-Protection",
        "1; mode=block".parse().unwrap(),
    );
}
```

## Rate Limiting

### 1. Request Rate Limiting

```rust
use tower::limit::RateLimitLayer;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

pub fn create_rate_limiter() -> GovernorLayer {
    let config = GovernorConfigBuilder::default()
        .per_second(100)
        .burst_size(200)
        .finish()
        .unwrap();
    
    GovernorLayer::new(config)
}
```

### 2. User-Based Rate Limiting

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct UserRateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl UserRateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }
    
    pub async fn check_rate_limit(&self, user_id: &str) -> Result<(), RevisionError> {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();
        
        let user_requests = requests.entry(user_id.to_string()).or_insert_with(Vec::new);
        
        // Remove old requests
        user_requests.retain(|&t| now.duration_since(t) < self.window);
        
        // Check limit
        if user_requests.len() >= self.max_requests {
            return Err(RevisionError::RateLimitExceeded);
        }
        
        user_requests.push(now);
        Ok(())
    }
}
```

## Audit Logging

### 1. Log All Security Events

```rust
use tracing::{info, warn, error};

pub fn log_security_event(event: SecurityEvent) {
    match event {
        SecurityEvent::LoginSuccess { user_id, ip } => {
            info!(
                event = "login_success",
                user_id = user_id,
                ip = ip,
                "User logged in successfully"
            );
        }
        SecurityEvent::LoginFailure { user_id, ip, reason } => {
            warn!(
                event = "login_failure",
                user_id = user_id,
                ip = ip,
                reason = reason,
                "Login attempt failed"
            );
        }
        SecurityEvent::UnauthorizedAccess { user_id, resource } => {
            error!(
                event = "unauthorized_access",
                user_id = user_id,
                resource = resource,
                "Unauthorized access attempt"
            );
        }
        SecurityEvent::RevisionCreated { revision_id, user_id } => {
            info!(
                event = "revision_created",
                revision_id = revision_id,
                user_id = user_id,
                "Revision created"
            );
        }
        SecurityEvent::RevisionRestored { revision_id, user_id } => {
            info!(
                event = "revision_restored",
                revision_id = revision_id,
                user_id = user_id,
                "Revision restored"
            );
        }
    }
}
```

### 2. Immutable Audit Log

```rust
pub struct AuditLog {
    db: DatabaseConnection,
}

impl AuditLog {
    pub async fn record(&self, event: AuditEvent) -> Result<(), RevisionError> {
        // Insert into audit_log table
        let audit_entry = audit_log::ActiveModel {
            id: Set(Uuid::new_v4()),
            event_type: Set(event.event_type()),
            user_id: Set(event.user_id()),
            timestamp: Set(Utc::now()),
            details: Set(serde_json::to_value(event)?),
            ip_address: Set(event.ip_address()),
            user_agent: Set(event.user_agent()),
        };
        
        audit_log::Entity::insert(audit_entry)
            .exec(&self.db)
            .await?;
        
        Ok(())
    }
}
```

## Compliance

### 1. GDPR Compliance

**Right to Access:**

```rust
pub async fn export_user_data(
    user_id: &str,
) -> Result<UserDataExport, RevisionError> {
    let revisions = ContentRevisions::find()
        .filter(content_revisions::Column::CreatedBy.eq(user_id))
        .all(&db)
        .await?;
    
    Ok(UserDataExport {
        user_id: user_id.to_string(),
        revisions,
        exported_at: Utc::now(),
    })
}
```

**Right to Erasure:**

```rust
pub async fn delete_user_data(
    user_id: &str,
) -> Result<u64, RevisionError> {
    // Delete all revisions created by user
    let result = ContentRevisions::delete_many()
        .filter(content_revisions::Column::CreatedBy.eq(user_id))
        .exec(&db)
        .await?;
    
    // Anonymize remaining references
    ContentRevisions::update_many()
        .col_expr(
            content_revisions::Column::CreatedBy,
            Expr::value("anonymized"),
        )
        .filter(content_revisions::Column::CreatedBy.eq(user_id))
        .exec(&db)
        .await?;
    
    Ok(result.rows_affected)
}
```

**Data Retention:**

```rust
impl ContentRevisionConfig for UserData {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // GDPR: keep data only as long as necessary
        Some(RetentionPolicy::KeepDays { days: 365 })
    }
}
```

### 2. SOC 2 Compliance

**Access Controls:**

```rust
// Enforce principle of least privilege
pub fn check_access(claims: &Claims, resource: &str) -> bool {
    match resource {
        "revisions:read" => claims.roles.contains(&"viewer".to_string()),
        "revisions:write" => claims.roles.contains(&"editor".to_string()),
        "revisions:delete" => claims.roles.contains(&"admin".to_string()),
        _ => false,
    }
}
```

**Audit Trail:**

```rust
// Log all access to sensitive data
pub async fn access_sensitive_data(
    user_id: &str,
    resource_id: &str,
) -> Result<(), RevisionError> {
    audit_log.record(AuditEvent::DataAccess {
        user_id: user_id.to_string(),
        resource_id: resource_id.to_string(),
        timestamp: Utc::now(),
    }).await?;
    
    Ok(())
}
```

## Security Checklist

### Pre-Deployment Checklist

- [ ] **Authentication**
  - [ ] JWT secret is strong and secure
  - [ ] JWT expiration is set (e.g., 24 hours)
  - [ ] API keys are generated securely
  - [ ] OAuth credentials are stored securely

- [ ] **Authorization**
  - [ ] RBAC is implemented
  - [ ] Tenant isolation is enforced
  - [ ] Resource ownership is checked
  - [ ] Admin actions require admin role

- [ ] **Data Protection**
  - [ ] Encryption at rest is enabled
  - [ ] Encryption in transit (TLS) is configured
  - [ ] Sensitive data is masked in logs
  - [ ] Database credentials are secure

- [ ] **Input Validation**
  - [ ] All inputs are validated
  - [ ] User input is sanitized
  - [ ] JSON schema validation is enabled
  - [ ] File uploads are restricted

- [ ] **SQL Injection**
  - [ ] All queries use parameterized statements
  - [ ] ORM is used where possible
  - [ ] No string concatenation in SQL

- [ ] **XSS Prevention**
  - [ ] Output is escaped
  - [ ] Content Security Policy is set
  - [ ] HTTP security headers are configured

- [ ] **Rate Limiting**
  - [ ] Request rate limiting is enabled
  - [ ] User-based rate limiting is implemented
  - [ ] DDoS protection is in place

- [ ] **Audit Logging**
  - [ ] All security events are logged
  - [ ] Audit log is immutable
  - [ ] Logs are stored securely
  - [ ] Log retention policy is set

- [ ] **Compliance**
  - [ ] GDPR compliance is implemented
  - [ ] Data retention policies are set
  - [ ] User data export is available
  - [ ] User data deletion is available

- [ ] **Infrastructure**
  - [ ] Firewall rules are configured
  - [ ] Database is not publicly accessible
  - [ ] Backups are encrypted
  - [ ] Monitoring and alerting is set up

### Post-Deployment Checklist

- [ ] **Monitoring**
  - [ ] Security metrics are collected
  - [ ] Alerts are configured
  - [ ] Logs are monitored
  - [ ] Anomalies are detected

- [ ] **Incident Response**
  - [ ] Incident response plan is documented
  - [ ] Security team is trained
  - [ ] Communication plan is ready
  - [ ] Forensics tools are available

- [ ] **Regular Reviews**
  - [ ] Security audits are scheduled
  - [ ] Penetration testing is planned
  - [ ] Vulnerability scanning is automated
  - [ ] Dependencies are updated regularly

## Заключение

Этот security audit guide покрывает:

✅ **Threat Model** — идентификация угроз  
✅ **Authentication** — JWT, API keys, OAuth  
✅ **Authorization** — RBAC, resource-based, ABAC  
✅ **Data Protection** — encryption, masking, key management  
✅ **Input Validation** — validation, sanitization, schema  
✅ **SQL Injection** — parameterized queries, ORM  
✅ **XSS Prevention** — output escaping, CSP  
✅ **Rate Limiting** — request and user-based  
✅ **Audit Logging** — security events, immutable logs  
✅ **Compliance** — GDPR, SOC 2  
✅ **Security Checklist** — pre и post deployment  

Следуя этому guide, вы сможете обеспечить:

- **Confidentiality** — защита от несанкционированного доступа
- **Integrity** — защита от изменения данных
- **Availability** — защита от DoS атак
- **Compliance** — соответствие регуляторным требованиям

**Безопасность — это непрерывный процесс!** 🔒
