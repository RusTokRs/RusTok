#!/usr/bin/env node
// Fast source-level guardrails for the rustok-auth admin FFA boundary.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) {
    failures.push(`${relativePath}: expected auth admin FFA file`);
  }
}

function assertContains(text, marker, description) {
  if (!text.includes(marker)) failures.push(description);
}

function assertNotContains(text, marker, description) {
  if (text.includes(marker)) failures.push(description);
}

const corePath = "crates/rustok-auth/admin/src/core.rs";
const mutationPortPath = "crates/rustok-auth/src/admin_mutations.rs";
const restContractPath = "crates/rustok-auth/src/rest.rs";
const serverAuthControllerPath = "apps/server/src/controllers/auth.rs";
const serverOauthControllerPath = "apps/server/src/controllers/oauth.rs";
const serverUsersControllerPath = "apps/server/src/controllers/users.rs";
const authProviderPath = "apps/server/src/services/auth_admin_mutation_provider.rs";
const lifecycleProviderPath = "apps/server/src/services/auth_lifecycle_provider.rs";
const runtimeExtensionsPath = "apps/server/src/services/module_event_dispatcher.rs";
const authGraphqlPath = "crates/rustok-auth/src/graphql/auth_mutation.rs";
const oauthGraphqlPath = "crates/rustok-auth/src/graphql/mutation.rs";
const authGraphqlModPath = "crates/rustok-auth/src/graphql/mod.rs";
const userGraphqlPath = "apps/server/src/graphql/mutations.rs";
const transportPath = "crates/rustok-auth/admin/src/transport/mod.rs";
const nativeTransportPath = "crates/rustok-auth/admin/src/transport/native_server_adapter.rs";
const uiPath = "crates/rustok-auth/admin/src/ui/users.rs";
const detailUiPath = "crates/rustok-auth/admin/src/ui/user_details.rs";
const oauthUiPath = "crates/rustok-auth/admin/src/ui/oauth_apps.rs";
const loginUiPath = "crates/rustok-auth/admin/src/ui/login.rs";
const registerUiPath = "crates/rustok-auth/admin/src/ui/register.rs";
const resetUiPath = "crates/rustok-auth/admin/src/ui/reset.rs";
const profileUiPath = "crates/rustok-auth/admin/src/ui/profile.rs";
const securityUiPath = "crates/rustok-auth/admin/src/ui/security.rs";
const authAdminUiPath = "crates/rustok-auth/admin/src/ui/auth_admin.rs";
const modelPath = "crates/rustok-auth/admin/src/model.rs";
const i18nPath = "crates/rustok-auth/admin/src/i18n.rs";
const planPath = "crates/rustok-auth/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [corePath, mutationPortPath, restContractPath, serverAuthControllerPath, serverOauthControllerPath, serverUsersControllerPath, authProviderPath, lifecycleProviderPath, runtimeExtensionsPath, authGraphqlPath, oauthGraphqlPath, authGraphqlModPath, userGraphqlPath, transportPath, nativeTransportPath, uiPath, detailUiPath, oauthUiPath, loginUiPath, registerUiPath, resetUiPath, profileUiPath, securityUiPath, authAdminUiPath, modelPath, i18nPath, planPath, registryPath, packagePath]) {
  assertExists(filePath);
}

const core = readRepo(corePath);
const mutationPort = readRepo(mutationPortPath);
const restContract = readRepo(restContractPath);
const serverAuthController = readRepo(serverAuthControllerPath);
const serverOauthController = readRepo(serverOauthControllerPath);
const serverUsersController = readRepo(serverUsersControllerPath);
const authProvider = readRepo(authProviderPath);
const lifecycleProvider = readRepo(lifecycleProviderPath);
const runtimeExtensions = readRepo(runtimeExtensionsPath);
const authGraphql = readRepo(authGraphqlPath);
const oauthGraphql = readRepo(oauthGraphqlPath);
const authGraphqlMod = readRepo(authGraphqlModPath);
const userGraphql = readRepo(userGraphqlPath);
const transport = readRepo(transportPath);
const nativeTransport = readRepo(nativeTransportPath);
const ui = readRepo(uiPath);
const detailUi = readRepo(detailUiPath);
const oauthUi = readRepo(oauthUiPath);
const loginUi = readRepo(loginUiPath);
const registerUi = readRepo(registerUiPath);
const resetUi = readRepo(resetUiPath);
const profileUi = readRepo(profileUiPath);
const securityUi = readRepo(securityUiPath);
const authAdminUi = readRepo(authAdminUiPath);
const model = readRepo(modelPath);
const i18n = readRepo(i18nPath);
const plan = readRepo(planPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

for (const marker of ["user_list_page", "user_list_query_params", "user_list_pagination", "user_list_previous_page", "UserListPagination", "prepare_create_user_input", "prepare_update_user_input", "CreateUserInputError", "graphql_user_view", "GraphqlUserViewModel", "UserEditFormValues", "oauth_app_type_defaults", "prepare_create_oauth_app_input", "prepare_update_oauth_app_input", "format_oauth_app_timestamp", "oauth_app_list_item_view", "OAuthAppListItemViewModel", "prepare_login_request", "prepare_register_request", "prepare_password_reset_request", "prepare_change_password_request", "ChangePasswordInputError", "prepare_profile_name", "initial_profile_preferred_locale", "classify_auth_transport_error", "AuthTransportErrorKind"]) {
  assertContains(core, marker, `${corePath}: missing core-owned helper ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "spawn_local", "GraphqlRequest"]) {
  assertNotContains(core, marker, `${corePath}: core must stay framework and transport free (${marker})`);
}
for (const marker of ["pub trait UserAdminMutationPort", "pub struct UserAdminMutationRuntime", "pub trait OAuthAdminPort", "pub struct OAuthAdminRuntime", "pub struct AuthAdminMutationContext", "async fn create_user", "async fn update_user", "async fn delete_user", "async fn create_oauth_app", "async fn update_oauth_app", "async fn rotate_oauth_app_secret", "async fn revoke_oauth_app"]) {
  assertContains(mutationPort, marker, `${mutationPortPath}: missing auth-owned mutation boundary marker ${marker}`);
}
for (const marker of [
  "pub struct LoginParams",
  "pub struct RefreshRequest",
  "pub struct RegisterParams",
  "pub struct AcceptInviteParams",
  "pub struct InviteAcceptResponse",
  "pub struct RequestResetParams",
  "pub struct ConfirmResetParams",
  "pub struct RequestVerificationParams",
  "pub struct ConfirmVerificationParams",
  "pub struct ChangePasswordParams",
  "pub struct UpdateProfileParams",
  "pub struct ResetRequestResponse",
  "pub struct VerificationRequestResponse",
  "pub struct GenericStatusResponse",
  "pub struct SessionItem",
  "pub struct SessionsResponse",
  "pub struct SessionListParams",
  "pub struct UserResponse",
  "pub struct UserInfo",
  "pub struct AuthResponse",
  "pub struct LogoutResponse",
]) {
  assertContains(restContract, marker, `${restContractPath}: missing auth-owned REST DTO marker ${marker}`);
  assertNotContains(serverAuthController, marker, `${serverAuthControllerPath}: auth REST DTO ownership must stay in rustok-auth (${marker})`);
}
assertContains(restContract, "utoipa::ToSchema", `${restContractPath}: auth owner REST DTOs must preserve OpenAPI schema derives`);
assertContains(serverAuthController, "pub use rustok_auth::{", `${serverAuthControllerPath}: server controller should re-export owner REST DTOs for Swagger compatibility`);
assertContains(serverAuthController, "user_response_from_model", `${serverAuthControllerPath}: server controller may map persisted users into owner REST DTOs`);
assertNotContains(serverAuthController, "utoipa::ToSchema", `${serverAuthControllerPath}: server controller must not own auth OpenAPI DTO derives`);
assertNotContains(serverAuthController, "UserResponse::from_user_and_role", `${serverAuthControllerPath}: server controller must not rely on a host-owned UserResponse constructor`);
for (const marker of [
  "pub struct TokenRequest",
  "pub struct AuthorizeRequest",
  "pub struct BrowserAuthorizeRequest",
  "pub struct ConsentRequest",
  "pub struct BrowserSessionResponse",
  "pub struct TokenResponse",
  "pub struct TokenErrorResponse",
  "pub struct RevokeRequest",
]) {
  assertContains(restContract, marker, `${restContractPath}: missing auth-owned OAuth REST DTO marker ${marker}`);
  assertNotContains(serverOauthController, marker, `${serverOauthControllerPath}: OAuth REST DTO ownership must stay in rustok-auth (${marker})`);
}
assertContains(serverOauthController, "use rustok_auth::{", `${serverOauthControllerPath}: OAuth HTTP adapter must import owner REST DTOs`);
assertContains(serverOauthController, "fn oauth_error_response(error: TokenErrorResponse)", `${serverOauthControllerPath}: server may keep HTTP status mapping for owner OAuth error DTO`);
assertNotContains(serverOauthController, "impl axum::response::IntoResponse for TokenErrorResponse", `${serverOauthControllerPath}: server must not implement external HTTP traits for owner OAuth DTOs`);
assertNotContains(serverOauthController, "use serde::{Deserialize, Serialize}", `${serverOauthControllerPath}: OAuth controller must not own serde DTO definitions`);
for (const marker of [
  "pub struct UserItem",
  "pub struct UsersResponse",
  "pub struct UsersListParams",
]) {
  assertContains(restContract, marker, `${restContractPath}: missing auth-owned users REST DTO marker ${marker}`);
  assertNotContains(serverUsersController, marker, `${serverUsersControllerPath}: users REST DTO ownership must stay in rustok-auth (${marker})`);
}
assertContains(serverUsersController, "use rustok_auth::{UserItem, UsersListParams, UsersResponse};", `${serverUsersControllerPath}: users HTTP adapter must import owner REST DTOs`);
assertNotContains(serverUsersController, "use serde::{Deserialize, Serialize}", `${serverUsersControllerPath}: users controller must not own serde DTO definitions`);
assertNotContains(serverUsersController, "use utoipa::ToSchema", `${serverUsersControllerPath}: users controller must not own OpenAPI DTO derives`);
for (const marker of ["impl OAuthAdminPort for ServerAuthAdminMutationProvider", "impl UserAdminMutationPort for ServerAuthAdminMutationProvider", "OAuthAppService::create_app", "OAuthAppService::update_app", "OAuthAppService::rotate_secret", "OAuthAppService::revoke_app", "AuthLifecycleService::create_user_in_tx", "RbacService::has_any_permission", "FlexAttachedValuesService::prepare_update"]) {
  assertContains(authProvider, marker, `${authProviderPath}: missing shared server auth mutation provider marker ${marker}`);
}
for (const marker of ["impl AuthLifecyclePort for ServerAuthLifecycleProvider", "AuthLifecycleService::login", "AuthLifecycleService::register", "AuthLifecycleService::refresh", "AuthLifecycleService::list_sessions", "email_service_from_ctx", "encode_password_reset_token"]) {
  assertContains(lifecycleProvider, marker, `${lifecycleProviderPath}: missing auth lifecycle provider marker ${marker}`);
}
for (const marker of [".begin()", ".update(&tx)", "persist_localized_values(\n                &tx", "tx.commit()"]) {
  assertContains(authProvider, marker, `${authProviderPath}: create user custom-field lifecycle must stay atomic with the shared provider transaction (${marker})`);
}
for (const marker of ["fn parse_user_role", "value.trim().to_ascii_lowercase()", ".map(parse_user_role)", "parses_admin_user_enums_case_insensitively"]) {
  assertContains(authProvider, marker, `${authProviderPath}: shared provider must normalize admin user role/status enums from native and GraphQL adapters (${marker})`);
}
assertNotContains(authProvider, "UserRole::from_str)\n            .transpose()\n            .map_err(map_custom_field_error)", `${authProviderPath}: user role parse errors must map to validation errors, not custom-field errors`);
for (const marker of ["build_shared_runtime_extensions_with_host_providers", "AuthLifecycleRuntime::new", "OAuthAdminRuntime::new", "UserAdminMutationRuntime::new", "ServerAuthAdminMutationProvider::new", "ServerAuthLifecycleProvider::new"]) {
  assertContains(runtimeExtensions, marker, `${runtimeExtensionsPath}: missing auth provider registration marker ${marker}`);
}
for (const marker of ["AuthLifecycleRuntime", "AuthQuery", "AuthMutation", "OAuthAdminRuntime"]) {
  assertContains(authGraphqlMod, marker, `${authGraphqlModPath}: owner auth GraphQL must expose lifecycle runtime/root marker ${marker}`);
}
for (const marker of [".sign_in(", ".sign_up(", ".refresh_token(", ".forgot_password(", ".update_profile(", ".change_password(", ".reset_password(", ".logout(", ".revoke_session(", ".revoke_all_sessions(", ".accept_invite("]) {
  assertContains(authGraphql, marker, `${authGraphqlPath}: GraphQL auth mutations must consume shared lifecycle provider (${marker})`);
}
for (const marker of ["AuthLifecycleService::login", "AuthLifecycleService::register", "email_service_from_ctx", "crate::services", "crate::models", "loco_rs", "sea_orm"]) {
  assertNotContains(authGraphql, marker, `${authGraphqlPath}: owner GraphQL must not bypass shared lifecycle provider (${marker})`);
}
for (const marker of [".create_oauth_app(", ".update_oauth_app(", ".rotate_oauth_app_secret(", ".revoke_oauth_app("]) {
  assertContains(oauthGraphql, marker, `${oauthGraphqlPath}: GraphQL OAuth mutations must consume shared provider (${marker})`);
}
for (const marker of ["OAuthAppService::create_app", "OAuthAppService::update_app", "OAuthAppService::rotate_secret", "OAuthAppService::revoke_app"]) {
  assertNotContains(oauthGraphql, marker, `${oauthGraphqlPath}: GraphQL adapter must not bypass shared OAuth mutation provider (${marker})`);
}
for (const marker of ["UserAdminMutationRuntime", ".create_user(", ".update_user(", ".delete_user("]) {
  assertContains(userGraphql, marker, `${userGraphqlPath}: GraphQL user mutations must consume shared provider (${marker})`);
}
for (const marker of ["AuthLifecycleService::create_user", "RbacService::replace_user_role", "FlexAttachedValuesService::delete_localized_values"]) {
  assertNotContains(userGraphql, marker, `${userGraphqlPath}: GraphQL user adapter must not bypass shared provider (${marker})`);
}
for (const marker of ["create_user_native", "update_user_native", "delete_user_native"]) {
  assertContains(nativeTransport, marker, `${nativeTransportPath}: missing native user mutation adapter marker ${marker}`);
  assertContains(transport, marker, `${transportPath}: user facade must be native-first (${marker})`);
}
assertContains(nativeTransport, "UserAdminMutationRuntime", `${nativeTransportPath}: native user mutations must consume the shared runtime`);
for (const marker of ["leptos_axum::extract::<rustok_api::RequestContext>()", "leptos_axum::extract::<rustok_api::TenantContext>()", "tenant_context.default_locale", "locale,"]) {
  assertContains(nativeTransport, marker, `${nativeTransportPath}: native user mutation context must consume host-resolved locale (${marker})`);
}
for (const marker of ["leptos::", "sea_orm::", "loco_rs::", "apps::server"] ) {
  assertNotContains(mutationPort, marker, `${mutationPortPath}: mutation port must remain host and framework independent (${marker})`);
}
if (/derive\([^)]*Debug[^)]*\)\]\s*pub struct CreateUserCommand/.test(mutationPort)) {
  failures.push(`${mutationPortPath}: password-bearing create command must not derive Debug`);
}
for (const marker of ["fetch_users", "create_user", "fetch_user", "update_user_details"]) {
  assertContains(transport, marker, `${transportPath}: missing transport facade marker ${marker}`);
}
assertNotContains(ui, "CreateUserInput {", `${uiPath}: create DTO construction must remain core-owned`);
assertNotContains(detailUi, "UpdateUserInput {", `${detailUiPath}: update DTO construction must remain core-owned`);
assertContains(ui, "prepare_create_user_input", `${uiPath}: users UI must consume core create helper`);
assertContains(detailUi, "prepare_update_user_input", `${detailUiPath}: detail UI must consume core update helper`);
assertContains(ui, "graphql_user_view", `${uiPath}: users UI must consume shared user view mapping`);
assertContains(detailUi, "graphql_user_view", `${detailUiPath}: detail UI must consume shared user view mapping`);
for (const [filePath, source] of [[uiPath, ui], [detailUiPath, detailUi]]) {
  for (const marker of ["user.name.clone().unwrap_or_default()", "user.status.eq_ignore_ascii_case", "format!(\"/users/{}\""]) {
    assertNotContains(source, marker, `${filePath}: user presentation policy must remain core-owned (${marker})`);
  }
}
assertNotContains(oauthUi, "CreateOAuthAppInput {", `${oauthUiPath}: OAuth create DTO construction must remain core-owned`);
assertNotContains(oauthUi, "UpdateOAuthAppInput {", `${oauthUiPath}: OAuth update DTO construction must remain core-owned`);
assertContains(oauthUi, "prepare_create_oauth_app_input", `${oauthUiPath}: OAuth UI must consume core create helper`);
assertContains(oauthUi, "prepare_update_oauth_app_input", `${oauthUiPath}: OAuth UI must consume core update helper`);
assertContains(oauthUi, "oauth_app_list_item_view", `${oauthUiPath}: OAuth UI must consume core list-item view mapping`);
for (const marker of ["app.scopes.join(\", \")", "app.grant_types.join(\", \")", "app.managed_by_manifest", "format_oauth_app_timestamp(app.last_used_at)"]) {
  assertNotContains(oauthUi, marker, `${oauthUiPath}: OAuth list presentation policy must remain core-owned (${marker})`);
}
for (const marker of ["pub struct CreateOAuthAppInput", "pub struct UpdateOAuthAppInput"]) {
  assertContains(model, marker, `${modelPath}: missing framework-neutral OAuth DTO ${marker}`);
  assertNotContains(transport, marker, `${transportPath}: OAuth DTO ownership must stay in model (${marker})`);
}
assertContains(transport, "pub use crate::model::{CreateOAuthAppInput, UpdateOAuthAppInput};", `${transportPath}: transport must preserve OAuth DTO compatibility re-exports`);
for (const marker of ["rustok-admin-locale", "LocalStorage::get", "get_stored_locale"]) {
  assertNotContains(transport, marker, `${transportPath}: auth transport must not read package-local locale storage (${marker})`);
}
for (const [filePath, source, helper] of [
  [loginUiPath, loginUi, "prepare_login_request"],
  [registerUiPath, registerUi, "prepare_register_request"],
  [resetUiPath, resetUi, "prepare_password_reset_request"],
  [profileUiPath, profileUi, "prepare_profile_name"],
]) {
  assertContains(source, helper, `${filePath}: UI must consume core helper ${helper}`);
  assertNotContains(source, ".trim()", `${filePath}: request normalization must remain core-owned`);
}
assertContains(profileUi, "initial_profile_preferred_locale", `${profileUiPath}: profile UI must derive preference defaults through core-owned host-locale policy`);
assertNotContains(profileUi, "String::from(\"ru\")", `${profileUiPath}: profile UI must not hardcode a package-local locale default`);
for (const marker of ["err_str.contains(\"Unauthorized\")", "err_str.contains(\"HTTP\")", "err_str.contains(\"Network\")"]) {
  assertNotContains(profileUi, marker, `${profileUiPath}: profile transport error policy must remain core-owned (${marker})`);
  assertNotContains(securityUi, marker, `${securityUiPath}: security transport error policy must remain core-owned (${marker})`);
}
assertContains(securityUi, "prepare_change_password_request", `${securityUiPath}: security UI must consume core change-password request helper`);
assertNotContains(securityUi, "current_password.get().is_empty()", `${securityUiPath}: password validation must remain core-owned`);

for (const marker of ["include_str!(\"../locales/en.json\")", "include_str!(\"../locales/ru.json\")", "MESSAGES.t_for_locale"]) {
  assertContains(i18n, marker, `${i18nPath}: missing host-locale catalog marker ${marker}`);
}
assertContains(i18n, "auth_transport_error_message", `${i18nPath}: missing shared localized transport-error mapping`);
for (const [filePath, source] of [[uiPath, ui], [detailUiPath, detailUi], [profileUiPath, profileUi], [securityUiPath, securityUi]]) {
  assertContains(source, "auth_transport_error_message", `${filePath}: UI must consume shared localized transport-error mapping`);
  assertNotContains(source, "format!(\"{:?}\", e)", `${filePath}: raw mutation errors must not be rendered directly`);
}
for (const marker of ["authAdmin.title", "authAdmin.usersTitle", "authAdmin.oauthTitle", "authAdmin.profileTitle", "authAdmin.securityTitle"]) {
  assertContains(authAdminUi, marker, `${authAdminUiPath}: auth admin landing copy must use host-locale catalog (${marker})`);
}
for (const marker of ["Identity & Access Control Panel", "User Accounts", "OAuth Connections", "Profile Settings", "Security & Sessions"]) {
  if (new RegExp(`>\\s*"${marker.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"\\s*<`).test(authAdminUi)) {
    failures.push(`${authAdminUiPath}: auth admin landing copy must not be hardcoded (${marker})`);
  }
}
assertContains(ui, "user_list_pagination", `${uiPath}: users UI must consume core pagination policy`);
for (const marker of ["page.get() <= 1", "page.get() * limit.get()", "(*value - 1).max(1)"]) {
  assertNotContains(ui, marker, `${uiPath}: pagination policy must remain core-owned (${marker})`);
}
assertContains(plan, "verify-auth-admin-boundary.mjs", `${planPath}: local plan must mention boundary guardrail`);
assertContains(registry, "verify-auth-admin-boundary.mjs", `${registryPath}: registry must mention boundary guardrail`);
assertContains(plan, "OAuthAdminPort", `${planPath}: local plan must document the auth-owned mutation boundary`);
assertContains(registry, "verify-auth-admin-boundary.mjs", `${registryPath}: registry must mention boundary guardrail`);
assertContains(plan, "FFA status: `phase_b_ready`", `${planPath}: local plan must record the closed Phase B boundary`);
assertContains(registry, "| `auth` | admin | `phase_b_ready`", `${registryPath}: registry must match the local auth FFA status`);
assertContains(packageJson, "verify:auth:admin-boundary", `${packagePath}: missing auth boundary script`);
assertContains(packageJson, "npm run verify:auth:admin-boundary", `${packagePath}: aggregate FFA verification must include auth boundary`);

if (failures.length > 0) {
  console.error("auth admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("auth admin boundary verification passed");
