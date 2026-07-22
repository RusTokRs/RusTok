import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

const files = {
  legacyNative: "crates/rustok-groups/admin/src/transport/native_applications_adapter.rs",
  casNative: "crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs",
  adminTransport: "crates/rustok-groups/admin/src/transport.rs",
  finalGraphql: "crates/rustok-groups/src/graphql_application_cas.rs",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups application boundary artifact: ${relative}`);
  }
}

if (failures.length === 0) {
  const legacyNative = read(files.legacyNative);
  const casNative = read(files.casNative);
  const adminTransport = read(files.adminTransport);
  const finalGraphql = read(files.finalGraphql);

  for (const marker of [
    'endpoint = "groups/admin/applications/policy/upsert"',
    "GroupApplicationCommandPort::upsert_group_application_policy",
    "GroupApplicationCommandPort::submit_group_membership_application",
    "groups_admin_upsert_application_policy_native",
  ]) {
    if (legacyNative.includes(marker)) {
      failures.push(`${files.legacyNative}: forbidden legacy native write marker ${JSON.stringify(marker)}`);
    }
  }

  for (const marker of [
    "GroupApplicationCasCommandPort",
    "upsert_group_application_policy_if_current",
    'endpoint = "groups/admin/applications/policy-if-current"',
  ]) {
    if (!casNative.includes(marker)) {
      failures.push(`${files.casNative}: missing CAS marker ${JSON.stringify(marker)}`);
    }
  }

  if (!adminTransport.includes("native_policy_locale_adapter::upsert_group_application_policy")) {
    failures.push(`${files.adminTransport}: admin policy writes must use the CAS native adapter`);
  }
  if (adminTransport.includes("native_applications_adapter::upsert_group_application_policy")) {
    failures.push(`${files.adminTransport}: admin transport routes policy writes through the legacy adapter`);
  }

  for (const marker of [
    "GroupsApplicationCasMutation",
    "upsert_group_application_policy_if_current",
    "submit_group_membership_application_if_current",
  ]) {
    if (!finalGraphql.includes(marker)) {
      failures.push(`${files.finalGraphql}: missing final CAS GraphQL marker ${JSON.stringify(marker)}`);
    }
  }
  for (const marker of [
    "async fn upsert_group_application_policy(\n",
    "async fn submit_group_membership_application(\n",
    "GroupsApplicationsMutation",
  ]) {
    if (finalGraphql.includes(marker)) {
      failures.push(`${files.finalGraphql}: final GraphQL root exposes legacy application mutation ${JSON.stringify(marker)}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Groups application native no-bypass verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups application native and GraphQL policy writes are CAS-only; legacy native policy write endpoint is absent.");
