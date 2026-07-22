import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const requireFile = (relative) => {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups invitation acceptance artifact: ${relative}`);
    return false;
  }
  return true;
};
const requireMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
    }
  }
};
const forbidMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) {
      failures.push(`${relative}: forbidden marker ${JSON.stringify(marker)}`);
    }
  }
};

const required = [
  "crates/rustok-groups/storefront/Cargo.toml",
  "crates/rustok-groups/storefront/src/core.rs",
  "crates/rustok-groups/storefront/src/model.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport/native_server_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/mod.rs",
  "crates/rustok-groups/storefront/src/ui/leptos.rs",
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
  "crates/rustok-groups/storefront/README.md",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
];
for (const relative of required) requireFile(relative);

requireMarkers("crates/rustok-groups/storefront/Cargo.toml", [
  "leptos-auth.workspace = true",
  "leptos-ui-routing.workspace = true",
  "dep:rustok-groups",
]);

requireMarkers("crates/rustok-groups/storefront/src/core.rs", [
  'GROUP_INVITATION_TOKEN_QUERY_KEY: &str = "invite"',
  "MIN_GROUP_INVITATION_TOKEN_LENGTH: usize = 32",
  "MAX_GROUP_INVITATION_TOKEN_LENGTH: usize = 160",
  "GroupsStorefrontInvitationInputError",
  "prepare_accept_group_invitation",
  "groups-storefront-accept-invitation-",
]);
forbidMarkers("crates/rustok-groups/storefront/src/core.rs", [
  "use leptos",
  "leptos::",
  "acceptGroupInvitation",
]);

requireMarkers("crates/rustok-groups/storefront/src/model.rs", [
  "AcceptGroupInvitationCommand",
  "GroupsStorefrontMembership",
  "GroupsStorefrontAcceptInvitationResult",
]);

requireMarkers("crates/rustok-groups/storefront/src/transport.rs", [
  "access_token: Option<String>",
  "graphql_with_access_token",
  "accept_groups_storefront_invitation",
  '"groups.storefront.invitation.accept"',
  "native_server_adapter::accept_invitation",
  "graphql_adapter::accept_invitation",
  'GROUPS_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back"',
]);

requireMarkers(
  "crates/rustok-groups/storefront/src/transport/native_server_adapter.rs",
  [
    "groups/storefront/invitations/accept",
    "AcceptGroupInvitationCommand",
    "AcceptGroupInvitationRequest",
    "GroupInvitationCommandPort",
    "GroupInvitationService",
    "AuthContext",
    "with_idempotency_key",
    "GroupsStorefrontAcceptInvitationResult",
  ],
);

requireMarkers(
  "crates/rustok-groups/storefront/src/transport/graphql_adapter.rs",
  [
    "GroupsStorefrontAcceptInvitation",
    "acceptGroupInvitation",
    "AcceptInvitationVariables",
    "access_token",
    "GroupsStorefrontAcceptInvitationResult",
  ],
);
forbidMarkers("crates/rustok-groups/storefront/src/transport/graphql_adapter.rs", [
  "createGroupInvitation",
  "revokeGroupInvitation",
]);

requireMarkers("crates/rustok-groups/storefront/src/ui/mod.rs", [
  "pub mod invitation_acceptance;",
]);
requireMarkers("crates/rustok-groups/storefront/src/ui/leptos.rs", [
  "leptos_auth::AuthContext",
  "use_context::<AuthContext>()",
  "AuthContext::get_token",
  "AuthContext::get_tenant",
  "graphql_with_access_token",
  "GroupsInvitationAcceptance",
  "transport=transport",
]);
requireMarkers(
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  [
    "prepare_accept_group_invitation",
    "accept_groups_storefront_invitation",
    "leptos_ui_routing::use_route_query_writer",
    "GROUP_INVITATION_TOKEN_QUERY_KEY",
    "query_writer.clear_key",
    'type="password"',
    'autocomplete="off"',
    'role="alert"',
    'role="status"',
  ],
);
forbidMarkers(
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  [
    "graphql_adapter",
    "native_server_adapter",
    "GroupInvitationService",
    "token_hash",
    "<code",
  ],
);

const localeKeys = [
  "groups.storefront.invitation.title",
  "groups.storefront.invitation.body",
  "groups.storefront.invitation.tokenLabel",
  "groups.storefront.invitation.tokenHint",
  "groups.storefront.invitation.accept",
  "groups.storefront.invitation.busy",
  "groups.storefront.invitation.error",
  "groups.storefront.invitation.success",
  "groups.storefront.invitation.group",
  "groups.storefront.invitation.role",
  "groups.storefront.invitation.status",
  "groups.storefront.invitation.missingToken",
  "groups.storefront.invitation.invalidToken",
];
for (const relative of [
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  let messages;
  try {
    messages = JSON.parse(read(relative));
  } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
    continue;
  }
  for (const key of localeKeys) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing invitation acceptance key ${key}`);
    }
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  let registry;
  try {
    registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  } catch (error) {
    failures.push(`Groups FBA registry is invalid JSON: ${error.message}`);
  }
  if (registry) {
    if (registry?.invitations?.storefront_acceptance_ui !== "implemented") {
      failures.push("Groups registry must declare storefront invitation acceptance UI");
    }
    if (registry?.invitations?.storefront_token_query_key !== "invite") {
      failures.push("Groups registry must lock the invitation query key");
    }
    if (registry?.invitations?.storefront_token_query_clear !== "on_submit") {
      failures.push("Groups registry must clear the invitation token query on submit");
    }
    if (registry?.invitations?.storefront_plaintext_result_rendering !== "never") {
      failures.push("Groups registry must forbid plaintext token result rendering");
    }
    if (registry?.invitations?.storefront_transport_fallback !== "never") {
      failures.push("Groups storefront invitation acceptance must reject fallback");
    }
    if (
      registry?.evidence?.invitation_acceptance_ui_static_boundary !==
      "scripts/verify/verify-groups-invitation-acceptance-ui.mjs"
    ) {
      failures.push("Groups invitation acceptance static evidence path is not registered");
    }
    if (registry?.evidence?.invitation_transport_parity !== null) {
      failures.push("Invitation transport parity must remain null before execution");
    }
  }
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "invitation acceptance/delivery source",
  "runtime parity and Notifications consumer evidence",
  "targeted invitation delivery remains `in_progress`",
]);

if (failures.length > 0) {
  console.error("Groups invitation acceptance UI verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups invitation acceptance UI verification passed.");
