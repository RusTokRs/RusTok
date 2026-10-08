import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../..");
const verifier = path.join(here, "verify-product-lifecycle-events.mjs");

const canonicalFiles = {
  "crates/libs/rustok-events/src/types/domain_event.rs": `pub enum DomainEvent {
    ProductPublished {
        product_id: Uuid,
    },
    ProductUnpublished {
        product_id: Uuid,
    },
    ProductArchived {
        product_id: Uuid,
    },
    ProductDeleted {
        product_id: Uuid,
    },
}

impl DomainEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ProductPublished { .. } => "product.published",
            Self::ProductUnpublished { .. } => "product.unpublished",
            Self::ProductArchived { .. } => "product.archived",
            Self::ProductDeleted { .. } => "product.deleted",
        }
    }

    pub fn schema_version(&self) -> u16 {
        match self {
            Self::ProductUnpublished { .. } => 1,
            Self::ProductArchived { .. } => 1,
        }
    }

    pub fn affects_index(&self) -> bool {
        matches!(
            self,
            Self::ProductPublished { .. }
                | Self::ProductUnpublished { .. }
                | Self::ProductArchived { .. }
                | Self::ProductDeleted { .. }
        )
    }
}
`,
  "crates/libs/rustok-events/src/types/validation.rs": `fn validate(&self) -> Result<(), String> {
    match self {
        Self::ProductPublished { product_id }
            | Self::ProductUnpublished { product_id }
            | Self::ProductArchived { product_id }
            | Self::ProductDeleted { product_id } => {
                validators::validate_not_nil_uuid("product_id", product_id)?;
                Ok(())
            }
    }
}
`,
  "crates/libs/rustok-events/src/schema.rs": `const PRODUCT_ID_FIELDS: &[FieldSchema] = &[field!("product_id", "uuid")];

pub const EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: "product.published",
        version: 1,
        description: "A product was published.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.unpublished",
        version: 1,
        description: "A product was unpublished.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.archived",
        version: 1,
        description: "A product was archived.",
        fields: PRODUCT_ID_FIELDS,
    },
];
`,
  "crates/libs/rustok-events/tests/canonical_contracts.rs": `fn sample_events() -> Vec<DomainEvent> {
    vec![
        DomainEvent::ProductPublished { product_id: id(26) },
        DomainEvent::ProductUnpublished { product_id: id(1013) },
        DomainEvent::ProductArchived { product_id: id(1014) },
    ]
}
`,
  "crates/modules/rustok-product/src/services/catalog/commands.rs": `pub async fn unpublish_product(tenant_id: Uuid, actor_id: Uuid, product_id: Uuid) {
    let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
    product_active.published_at = Set(None);
    txn.publish(
        tenant_id,
        Some(actor_id),
        DomainEvent::ProductUnpublished { product_id },
    )
    .await?;
}

pub async fn archive_product(tenant_id: Uuid, actor_id: Uuid, product_id: Uuid) {
    let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
    product_active.status = Set(entities::product::ProductStatus::Archived);
    product_active.published_at = Set(None);
    txn.publish(
        tenant_id,
        Some(actor_id),
        DomainEvent::ProductArchived { product_id },
    )
    .await?;
}

pub async fn update_product(tenant_id: Uuid, actor_id: Uuid, product_id: Uuid) {
    let will_archive = match input.status.as_ref() {
        Some(status) => *status == entities::product::ProductStatus::Archived,
        _ => false,
    };
        if will_become_active {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductPublished { product_id },
            )
            .await?;
        } else if will_archive {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductArchived { product_id },
            )
            .await?;
        } else if will_deactivate {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductUnpublished { product_id },
            )
            .await?;
        }
}

pub async fn delete_product(tenant_id: Uuid, actor_id: Uuid, product_id: Uuid) {}
`,
  "crates/modules/rustok-product/src/catalog_command_port.rs": `pub trait ProductCatalogCommandPort {
    async fn publish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError>;

    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError>;
}

impl ProductCatalogCommandPort for CatalogService {
    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let operation = "archive_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.archive_product(tenant_id, actor_id, product_id).await
    }
}
`,
  "crates/modules/rustok-product/src/media_asset_read_port.rs": `impl ProductCatalogCommandPort for ProductMediaValidatedCommandPort {
    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        self.inner.archive_product(context, product_id).await
    }
}
`,
  "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": `impl CommerceCatalogMutation {
    async fn unpublish_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        id: Uuid,
    ) -> Result<GqlProduct> {
        let port_context = product_command_context(ctx, (tenant_id, user_id), Some(id), idempotency_key, "unpublish_product")?;
        product_command_runtime(ctx)?
            .command_port()
            .unpublish_product(port_context.clone(), id)
            .await?;
        Ok(product.into())
    }

    async fn archive_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        id: Uuid,
    ) -> Result<GqlProduct> {
        let port_context = product_command_context(ctx, (tenant_id, user_id), Some(id), idempotency_key, "archive_product")?;
        product_command_runtime(ctx)?
            .command_port()
            .archive_product(port_context.clone(), id)
            .await?;
        Ok(product.into())
    }

    async fn delete_product(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        Ok(true)
    }
}
`,
  "crates/modules/rustok-commerce/src/controllers/products.rs": `pub async fn archive_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "archive_product",
    )?;
    let product = runtime
        .product_catalog_command_port()
        .archive_product(port_context.clone(), id)
        .await?;
    Ok(Json(product))
}
`,
  "crates/modules/rustok-commerce/src/controllers/admin/products.rs": `/// Archive admin ecommerce product
#[utoipa::path(
    post,
    path = "/admin/products/{id}/archive",
    tag = "admin"
)]
pub async fn archive_product(
    state: State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    path: Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::super::products::archive_product(state, tenant, auth, request_context, headers, path)
        .await
}
`,
  "crates/modules/rustok-commerce/src/controllers/admin/mod.rs": `.route(
    "/products/{id}/archive",
    axum::routing::post(products::archive_product),
)
`,
  "crates/modules/rustok-commerce/src/openapi.rs": `pub fn admin_paths() -> Vec<utoipa::openapi::path::PathItem> {
    vec![
        crate::controllers::admin::unpublish_product,
        crate::controllers::admin::archive_product,
    ]
}
`,
  "crates/modules/rustok-search/src/ingestion.rs": `impl EventHandler for SearchIngestion {
    fn handles(&self, event: &DomainEvent) -> bool {
        match event {
            DomainEvent::ProductCreated { .. }
            | DomainEvent::ProductUpdated { .. }
            | DomainEvent::ProductPublished { .. }
            | DomainEvent::ProductUnpublished { .. }
            | DomainEvent::ProductArchived { .. }
            | DomainEvent::ProductDeleted { .. } => true,
        }
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        match &envelope.event {
            DomainEvent::ProductCreated { product_id }
                | DomainEvent::ProductUpdated { product_id }
                | DomainEvent::ProductPublished { product_id }
                | DomainEvent::ProductUnpublished { product_id }
                | DomainEvent::ProductArchived { product_id } => {
                self.projector.upsert_product(envelope.tenant_id, *product_id).await
            }
        }
    }
}

fn project_step(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::ProductCreated { .. }
        | DomainEvent::ProductUpdated { .. }
        | DomainEvent::ProductPublished { .. }
        | DomainEvent::ProductUnpublished { .. }
        | DomainEvent::ProductArchived { .. } => "upsert_product",
    }
}
`,
  "crates/modules/rustok-product/src/services/index_refresh.rs": `pub(crate) fn product_locale_refresh_target(event: &DomainEvent) -> Option<Uuid> {
    match event {
        DomainEvent::ProductCreated { product_id }
        | DomainEvent::ProductUpdated { product_id }
        | DomainEvent::ProductPublished { product_id }
        | DomainEvent::ProductUnpublished { product_id }
        | DomainEvent::ProductArchived { product_id } => Some(*product_id),
        _ => None,
    }
}

/// Captures the canonical live and retained-delete state after one Product owner command.
`,
  "crates/modules/rustok-product/src/services/index_refresh_publication.rs": `let root_product_id = match &envelope.event {
    DomainEvent::ProductCreated { product_id }
    | DomainEvent::ProductUpdated { product_id }
    | DomainEvent::ProductPublished { product_id }
    | DomainEvent::ProductUnpublished { product_id }
    | DomainEvent::ProductArchived { product_id } => *product_id,
    _ => return Err(ProductIndexRefreshPublicationError::CausationMismatch),
};
if root_product_id != product_id {
`,
  "crates/modules/rustok-product/docs/product-unpublish-archive-events.md": `# Product unpublish and archive lifecycle events

\`product.unpublished\` clears storefront visibility, \`product.archived\` retires the product.

cargo run --locked -p rustok-events --example event_contract_digests -- --write
`,
  "docs/audits/product-module-engineering-audit-2026-10-07.md": `| \`PROD-EVENT-001\` | Устранено | Добавлен \`ProductUnpublished\` и команда архивации |
`,
};

const writeFixture = (overrides = {}) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "verify-lifecycle-events-"));
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

test("canonical lifecycle event contract passes", () => {
  const fixtureRoot = writeFixture();
  const result = runVerifier(fixtureRoot);
  assert.equal(result.status, 0, result.output);
  assert.match(result.output, /withdrawal and retirement are published/);
});

test("a dropped ProductUnpublished variant fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/libs/rustok-events/src/types/domain_event.rs": canonicalFiles[
      "crates/libs/rustok-events/src/types/domain_event.rs"
    ].replace(
      `    ProductUnpublished {
        product_id: Uuid,
    },
`,
      "",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /DomainEvent variant: missing/);
});

test("unpublish publishing the generic updated event fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product/src/services/catalog/commands.rs": canonicalFiles[
      "crates/modules/rustok-product/src/services/catalog/commands.rs"
    ].replace(
      "DomainEvent::ProductUnpublished { product_id },",
      "DomainEvent::ProductUpdated { product_id },",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /unpublish owner command: forbidden DomainEvent::ProductUpdated/);
});

test("archive missing from the owner port fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product/src/catalog_command_port.rs": canonicalFiles[
      "crates/modules/rustok-product/src/catalog_command_port.rs"
    ].replace(
      `    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError>;`,
      "",
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /command port trait: missing/);
});

test("archive GraphQL mutation without an idempotency key fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs": canonicalFiles[
      "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs"
    ].replace(
      `    async fn archive_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,`,
      `    async fn archive_product(
        &self,
        ctx: &Context<'_>,`,
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /archive GraphQL mutation: missing idempotency_key: String/);
});

test("a search consumer that ignores the new events fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-search/src/ingestion.rs": canonicalFiles[
      "crates/modules/rustok-search/src/ingestion.rs"
    ].replace(
      `        | DomainEvent::ProductUnpublished { .. }
        | DomainEvent::ProductArchived { .. } => "upsert_product",`,
      `        => "upsert_product",`,
    ),
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /search ingestion plan: missing/);
});

test("a lifecycle doc without the digest release gate fails the contract", () => {
  const fixtureRoot = writeFixture({
    "crates/modules/rustok-product/docs/product-unpublish-archive-events.md":
      "# Product unpublish and archive lifecycle events\n\n`product.unpublished` and `product.archived`.\n",
  });
  const result = runVerifier(fixtureRoot);
  assert.notEqual(result.status, 0);
  assert.match(result.output, /release gate/);
});
