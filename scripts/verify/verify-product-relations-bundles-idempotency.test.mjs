import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../..");
const verifier = path.join(here, "verify-product-relations-bundles-idempotency.mjs");

const canonicalFiles = {
  "crates/modules/rustok-product-relations/src/services/receipts.rs": `use rustok_outbox::idempotency;

pub const PRODUCT_RELATION_OWNER_SLUG: &str = "product_relations";
pub const CREATE_RELATION_OPERATION: &str = "create_relation";

pub struct ProductRelationCommandContext<'a> {
    pub idempotency_key: &'a str,
}

impl ProductRelationService {
    pub async fn create_relation_idempotent(
        &self,
        context: ProductRelationCommandContext<'_>,
        input: CreateProductRelationInput,
    ) -> Result<ProductRelationDto, ProductRelationCommandError> {
        let admission = idempotency::admit(
            self.database(),
            idempotency::OwnerOperationScope::Tenant(context.tenant_id),
            PRODUCT_RELATION_OWNER_SLUG,
            context.idempotency_key,
            CREATE_RELATION_OPERATION,
            &request,
        )
        .await?;
        let lease = match admission {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_relation_receipt(value),
            idempotency::Admission::ReplayError(error) => return Err(error.into()),
        };
        let txn = self.database().begin().await?;
        match create_relation_in_tx(&txn, context.tenant_id, input).await {
            Ok(relation) => {
                idempotency::complete(&txn, lease, &relation).await?;
                txn.commit().await?;
                Ok(relation)
            }
            Err(error) => {
                drop(txn);
                idempotency::fail(self.database(), lease, &port_error).await;
                Err(error.into())
            }
        }
    }
}

pub fn relation_command_error(error: &ProductRelationError) -> PortError {
    PortError::conflict("product_relation.already_exists", "product relation already exists")
}
`,
  "crates/modules/rustok-product-relations/src/services/relation_service.rs": `impl ProductRelationService {
    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }
}

pub(crate) async fn create_relation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: CreateProductRelationInput,
) -> ProductRelationResult<ProductRelationDto> {
    let model = active.insert(txn).await?;
    Ok(model.into())
}

impl ProductRelationsPort for ProductRelationService {
    async fn create_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: CreateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto> {
        let txn = self.db.begin().await?;
        let relation = create_relation_in_tx(&txn, tenant_id, input).await?;
        txn.commit().await?;
        Ok(relation)
    }

    async fn update_relation(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto> {
        Ok(relation.into())
    }
}
`,
  "crates/modules/rustok-product-relations/src/services/mod.rs": `pub mod receipts;
pub mod relation_service;

pub use receipts::{
    ProductRelationCommandContext, ProductRelationCommandError, relation_command_error,
};
pub use relation_service::ProductRelationService;
`,
  "crates/modules/rustok-product-relations/src/lib.rs": `pub use services::{
    ProductRelationCommandContext, ProductRelationCommandError, ProductRelationService,
    relation_command_error,
};
`,
  "crates/modules/rustok-product-relations/Cargo.toml": `[dependencies]
rustok-outbox.workspace = true
`,
  "crates/modules/rustok-product-relations/admin/src/transport/native_server_adapter.rs": `async fn product_relations_command_native(
    idempotency_key: String,
    command: ProductRelationsAdminCommand,
) -> Result<ProductRelationsAdminCommandResult, ServerFnError> {
    let created = service
        .create_relation_idempotent(
            ProductRelationCommandContext::new(
                tenant.id,
                Some(auth.user_id),
                idempotency_key.as_str(),
            ),
            input,
        )
        .await
        .map_err(|error| ServerFnError::new(relation_command_error_copy(&error)))?;
}
`,
  "crates/modules/rustok-product-bundles/src/services/receipts.rs": `use rustok_outbox::idempotency;

pub const PRODUCT_BUNDLE_OWNER_SLUG: &str = "product_bundles";
pub const CREATE_BUNDLE_OPERATION: &str = "create_bundle";
pub const ADD_BUNDLE_ITEM_OPERATION: &str = "add_bundle_item";

pub struct BundleCommandContext<'a> {
    pub idempotency_key: &'a str,
}

impl BundleService {
    pub async fn create_bundle_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        input: CreateBundleInput,
    ) -> Result<BundleDto, BundleCommandError> {
        let admission = idempotency::admit(
            service.database(),
            idempotency::OwnerOperationScope::Tenant(context.tenant_id),
            PRODUCT_BUNDLE_OWNER_SLUG,
            context.idempotency_key,
            CREATE_BUNDLE_OPERATION,
            &request,
        )
        .await?;
        let txn = self.database().begin().await?;
        match create_bundle_in_tx(&txn, context.tenant_id, input).await {
            Ok(bundle) => {
                idempotency::complete(&txn, lease, &bundle).await?;
                txn.commit().await?;
                Ok(bundle)
            }
            Err(error) => {
                drop(txn);
                idempotency::fail(service.database(), lease, &port_error).await;
                Err(error.into())
            }
        }
    }

    pub async fn add_bundle_item_idempotent(
        &self,
        context: BundleCommandContext<'_>,
        bundle_id: Uuid,
        item: BundleItemInput,
    ) -> Result<BundleItemDto, BundleCommandError> {
        let txn = self.database().begin().await?;
        match add_bundle_item_in_tx(&txn, context.tenant_id, bundle_id, item).await {
            Ok(created_item) => {
                idempotency::complete(&txn, lease, &created_item).await?;
                txn.commit().await?;
                Ok(created_item)
            }
            Err(error) => Err(error.into()),
        }
    }
}

pub fn bundle_command_error(error: &BundleError) -> PortError {
    match error {
        BundleError::SlugAlreadyExists(_) => {
            PortError::conflict("product_bundle.slug_conflict", "bundle slug conflict")
        }
        BundleError::ItemNotFound(_) => {
            PortError::not_found("product_bundle.item_not_found", "bundle item not found")
        }
        _ => PortError::unavailable("product_bundle.storage_unavailable", "unavailable"),
    }
}
`,
  "crates/modules/rustok-product-bundles/src/services/bundle_service.rs": `impl BundleService {
    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }
}

pub(crate) async fn create_bundle_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: CreateBundleInput,
) -> BundleResult<BundleDto> {
    let inserted_bundle = bundle_active.insert(txn).await?;
    Ok(BundleService::assemble_bundle_dto(inserted_bundle, vec![], vec![], None))
}

pub(crate) async fn add_bundle_item_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bundle_id: Uuid,
    item: BundleItemInput,
) -> BundleResult<BundleItemDto> {
    let inserted = active.insert(txn).await?;
    Ok(BundleItemDto {
        id: inserted.id,
    })
}

impl BundlePort for BundleService {
    async fn create_bundle(
        &self,
        tenant_id: Uuid,
        input: CreateBundleInput,
    ) -> BundleResult<BundleDto> {
        let txn = self.db.begin().await?;
        let bundle = create_bundle_in_tx(&txn, tenant_id, input).await?;
        txn.commit().await?;
        Ok(bundle)
    }

    async fn add_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item: BundleItemInput,
    ) -> BundleResult<BundleItemDto> {
        let txn = self.db.begin().await?;
        let created_item = add_bundle_item_in_tx(&txn, tenant_id, bundle_id, item).await?;
        txn.commit().await?;
        Ok(created_item)
    }
}
`,
  "crates/modules/rustok-product-bundles/src/services/mod.rs": `pub mod bundle_service;
pub mod receipts;

pub use bundle_service::BundleService;
pub use receipts::{BundleCommandContext, BundleCommandError, bundle_command_error};
`,
  "crates/modules/rustok-product-bundles/src/lib.rs": `pub use services::{
    BundleCommandContext, BundleCommandError, BundleService, bundle_command_error,
};
`,
  "crates/modules/rustok-product-bundles/Cargo.toml": `[dependencies]
rustok-outbox.workspace = true
`,
  "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs": `async fn bundle_command_native(
    idempotency_key: String,
    command: BundleAdminCommand,
) -> Result<BundleAdminCommandResult, ServerFnError> {
    match command {
        BundleAdminCommand::Create { draft } => {
            let created = service
                .create_bundle_idempotent(
                    BundleCommandContext::new(
                        tenant.id,
                        Some(auth.user_id),
                        idempotency_key.as_str(),
                    ),
                    input,
                )
                .await
                .map_err(|error| ServerFnError::new(bundle_command_error_copy(&error)))?;
        }
        BundleAdminCommand::AddItem { bundle_id } => {
            service
                .add_bundle_item_idempotent(
                    BundleCommandContext::new(
                        tenant.id,
                        Some(auth.user_id),
                        idempotency_key.as_str(),
                    ),
                    bundle_id,
                    item,
                )
                .await
                .map_err(|error| ServerFnError::new(bundle_command_error_copy(&error)))?;
        }
    }
}
`,
  "crates/modules/rustok-product-relations/admin/src/transport/graphql_adapter.rs": `const ADD_RELATION_MUTATION: &str = r#"
mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {
  addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {
    id
  }
}
"#;

pub async fn execute_command(
    token: Option<String>,
    tenant_slug: Option<String>,
    idempotency_key: String,
    command: ProductRelationsAdminCommand,
) -> Result<ProductRelationsAdminCommandResult, GraphqlProductRelationsAdminError> {
    match command {
        ProductRelationsAdminCommand::Add { draft } => {
            #[derive(Serialize)]
            struct Vars {
                #[serde(rename = "idempotencyKey")]
                idempotency_key: String,
                input: AddInput,
            }

            let vars = Vars {
                idempotency_key,
                input: AddInput {
                    product_id: draft.product_id,
                },
            };

            let data: AddResponse = request(ADD_RELATION_MUTATION, vars, token, tenant_slug).await?;
            Ok(ProductRelationsAdminCommandResult {
                item: Some(data.add_product_relation),
            })
        }
    }
}
`,
  "crates/modules/rustok-product-bundles/admin/src/transport/graphql_adapter.rs": `const CREATE_MUTATION: &str = r#"
mutation BundleAdminCreate($idempotencyKey: String!, $input: CreateBundleInputGql!) {
  createBundle(idempotencyKey: $idempotencyKey, input: $input) {
    id
  }
}
"#;

const ADD_ITEM_MUTATION: &str = r#"
mutation BundleAdminAddItem($idempotencyKey: String!, $bundleId: UUID!, $input: AddBundleItemInputGql!) {
  addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, input: $input) {
    id
  }
}
"#;

pub async fn execute_command(
    token: Option<String>,
    tenant_slug: Option<String>,
    idempotency_key: String,
    command: BundleAdminCommand,
) -> Result<BundleAdminCommandResult, GraphqlBundleAdminError> {
    match command {
        BundleAdminCommand::Create { draft } => {
            #[derive(Serialize)]
            struct Vars {
                #[serde(rename = "idempotencyKey")]
                idempotency_key: String,
                input: Input,
            }

            let vars = Vars {
                idempotency_key,
                input: Input {
                    slug: draft.slug,
                },
            };

            let data: CreateResponse = request(CREATE_MUTATION, vars, token, tenant_slug).await?;
            Ok(BundleAdminCommandResult {
                bundle: Some(data.bundle),
            })
        }
        BundleAdminCommand::AddItem { bundle_id } => {
            #[derive(Serialize)]
            struct Vars {
                #[serde(rename = "idempotencyKey")]
                idempotency_key: String,
                #[serde(rename = "bundleId")]
                bundle_id: String,
                input: Input,
            }

            let vars = Vars {
                idempotency_key,
                bundle_id: bundle_id.clone(),
                input: Input {
                    product_id,
                },
            };

            let _: serde_json::Value = request(ADD_ITEM_MUTATION, vars, token, tenant_slug).await?;
            Ok(BundleAdminCommandResult { success: true })
        }
    }
}
`,
  "apps/next-admin/packages/rustok-product/src/api/relations.ts": `export const ADD_RELATION_MUTATION = \`
mutation ProductAdminAddRelation($idempotencyKey: String!, $input: AddProductRelationInput!) {
  addProductRelation(idempotencyKey: $idempotencyKey, input: $input) {
    id
  }
}
\`;

export async function addProductRelation(
  opts: GqlOpts,
  input: AddProductRelationInput
): Promise<ProductRelation> {
  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();
  const data = await executor<
    { idempotencyKey: string; input: AddProductRelationInput },
    { addProductRelation: ProductRelation }
  >(
    ADD_RELATION_MUTATION,
    { idempotencyKey, input },
    opts.token,
    opts.tenantSlug
  );

  return data.addProductRelation;
}
`,
  "apps/next-admin/packages/rustok-product/src/api/bundles.ts": `const CREATE_BUNDLE_MUTATION = \`
mutation CreateBundle($idempotencyKey: String!, $input: CreateBundleInputGql!, $locale: String) {
  createBundle(idempotencyKey: $idempotencyKey, input: $input, locale: $locale) {
    id
  }
}
\`;

const ADD_BUNDLE_ITEM_MUTATION = \`
mutation AddBundleItem($idempotencyKey: String!, $bundleId: UUID!, $item: BundleItemInputGql!) {
  addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, item: $item) {
    id
  }
}
\`;

export async function createBundle(opts: GqlOpts, input: CreateBundleInput): Promise<ProductBundle> {
  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();
  const data = await executor<
    { idempotencyKey: string; input: unknown; locale?: string },
    { createBundle: ProductBundle }
  >(
    CREATE_BUNDLE_MUTATION,
    { idempotencyKey, input: { slug: input.slug }, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.createBundle;
}

export async function addBundleItem(opts: GqlOpts, input: AddBundleItemInput): Promise<BundleItem> {
  const executor = opts.graphql ?? graphqlRequest;
  const idempotencyKey = crypto.randomUUID();
  const data = await executor<
    { idempotencyKey: string; bundleId: string; item: unknown },
    { addBundleItem: BundleItem }
  >(
    ADD_BUNDLE_ITEM_MUTATION,
    { idempotencyKey, bundleId: input.bundleId, item: { productId: input.productId } },
    opts.token,
    opts.tenantSlug
  );

  return data.addBundleItem;
}
`,
  "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": `impl CommerceCatalogMutation {
    async fn add_product_relation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: AddProductRelationInput,
    ) -> Result<GqlProductRelation> {
        let caller_key = validate_scoped_idempotency_key(&idempotency_key, "Product relation")?;
        let scoped_key = scoped_catalog_operation_key(
            "commerce-graphql-relation",
            tenant_id,
            user_id,
            operation,
            None,
            caller_key,
        );
        let rel = service
            .create_relation_idempotent(
                ProductRelationCommandContext::new(tenant_id, Some(user_id), &scoped_key),
                domain_input,
            )
            .await
            .map_err(|error| relation_command_graphql_error(operation, error))?;
        Ok(rel.into())
    }

    async fn remove_product_relation(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        Ok(true)
    }

    async fn create_bundle(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: CreateBundleInputGql,
    ) -> Result<GqlBundle> {
        let caller_key = validate_scoped_idempotency_key(&idempotency_key, "Product bundle")?;
        let scoped_key = scoped_catalog_operation_key(
            "commerce-graphql-bundle",
            tenant_id,
            user_id,
            operation,
            None,
            caller_key,
        );
        let bundle = service
            .create_bundle_idempotent(
                BundleCommandContext::new(tenant_id, Some(user_id), &scoped_key),
                domain_input,
            )
            .await
            .map_err(|error| bundle_command_graphql_error(operation, error))?;
        Ok(bundle.into())
    }

    async fn update_bundle(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlBundle> {
        Ok(bundle.into())
    }

    async fn add_bundle_item(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        bundle_id: Uuid,
        item: BundleItemInputGql,
    ) -> Result<GqlBundleItem> {
        let scoped_key = scoped_catalog_operation_key(
            "commerce-graphql-bundle",
            tenant_id,
            user_id,
            operation,
            Some(bundle_id),
            caller_key,
        );
        let created_item = service
            .add_bundle_item_idempotent(
                BundleCommandContext::new(tenant_id, Some(user_id), &scoped_key),
                bundle_id,
                item,
            )
            .await
            .map_err(|error| bundle_command_graphql_error(operation, error))?;
        Ok(created_item.into())
    }

    async fn remove_bundle_item(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        Ok(true)
    }
}

fn catalog_write_command_error(domain: CatalogWriteDomain, operation: &'static str, error: PortError) -> async_graphql::Error {
    match error.code.as_str() {
        "outbox.operation_receipt_conflict" => "IDEMPOTENCY_KEY_CONFLICT",
        "outbox.operation_receipt_in_progress" => "CATALOG_OPERATION_IN_PROGRESS",
        _ => "CATALOG_OPERATION_FAILED",
    }
    async_graphql::Error::new("bounded")
}
`,
  "crates/modules/rustok-product/docs/write-boundary.md": `# Product write boundary

Every write must go through \`ProductCatalogCommandPort\` (14 operations including \`archive_product\`)
or \`ProductCatalogSchemaWritePort\`. Calling an owner service directly is not an allowed integration path.
Receipt-bound commands: create_relation_idempotent, create_bundle_idempotent,
add_bundle_item_idempotent. Shipping a new write family requires extending this document together
with the port.
`,
  "docs/modules/registry.md": `- \`product\`: \`scripts/verify/verify-product-relations-bundles-idempotency.mjs\`.
`,
  "docs/audits/product-module-engineering-audit-2026-10-07.md": `| \`PROD-IDEM-001\` | Устранено |
| \`PROD-PORT-001\` | Устранено |
`,
};

const writeFixture = (overrides = {}) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "verify-catalog-idem-"));
  const files = { ...canonicalFiles, ...overrides };
  for (const [relative, content] of Object.entries(files)) {
    if (content === null) continue;
    const target = path.join(fixtureRoot, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  }
  return fixtureRoot;
};

const runVerifier = (fixtureRoot) => {
  try {
    const stdout = execFileSync(process.execPath, [verifier], {
      cwd: repoRoot,
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: fixtureRoot },
      encoding: "utf8",
    });
    return { status: 0, output: stdout };
  } catch (error) {
    return { status: error.status ?? 1, output: `${error.stdout ?? ""}${error.stderr ?? ""}` };
  }
};

test("canonical receipt-bound relations and bundles pass", () => {
  const fixtureRoot = writeFixture();
  const result = runVerifier(fixtureRoot);
  assert.equal(result.status, 0, result.output);
  assert.match(result.output, /write through tenant-scoped owner receipts/);
});

test("a receipt that is never completed fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product-relations/src/services/receipts.rs": canonicalFiles[
      "crates/modules/rustok-product-relations/src/services/receipts.rs"
    ].replace("idempotency::complete(&txn, lease, &relation).await?;", ""),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /relations receipts: missing idempotency::complete/);
});

test("reusing the non-idempotent owner call fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": canonicalFiles[
      "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs"
    ].replace(".create_relation_idempotent(", ".create_relation(tenant_id, Some(user_id),"),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /forbidden non-idempotent create_relation call/);
});

test("an admin adapter that discards the caller key fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs":
      canonicalFiles[
        "crates/modules/rustok-product-bundles/admin/src/transport/native_server_adapter.rs"
      ].replace(
        "    match command {",
        "    let _ = idempotency_key;\n    match command {",
      ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /bundles native adapter: forbidden let _ = idempotency_key;/);
});

test("an optional GraphQL idempotency key fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": canonicalFiles[
      "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs"
    ].replace(
      `        idempotency_key: String,
        input: AddProductRelationInput,`,
      `        idempotency_key: Option<String>,
        input: AddProductRelationInput,`,
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /add_product_relation mutation: missing idempotency_key: String/);
});

test("a parallel receipt ledger fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product-relations/src/services/receipts.rs": `${canonicalFiles["crates/modules/rustok-product-relations/src/services/receipts.rs"]}
#[derive(DeriveEntityModel)]
#[sea_orm(table_name = "owner_operation_receipts")]
pub struct Model {}
`,
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /relations receipts: forbidden DeriveEntityModel/);
});

test("a Next admin client that stops sending its key fails the contract", () => {
  const fixtureRoot = writeFixture({
    "apps/next-admin/packages/rustok-product/src/api/bundles.ts": canonicalFiles[
      "apps/next-admin/packages/rustok-product/src/api/bundles.ts"
    ].replace(
      "addBundleItem(idempotencyKey: $idempotencyKey, bundleId: $bundleId, item: $item) {",
      "addBundleItem(bundleId: $bundleId, item: $item) {",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /next bundles client: missing addBundleItem/);
});

test("an undocumented write boundary fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product/docs/write-boundary.md": "# Product write boundary\n",
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /write boundary doc: missing/);
});
