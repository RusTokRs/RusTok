#!/usr/bin/env node
// Selected Product -> Pricing source contracts, not compilation, tests or remote execution.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const sourcePaths = {
  productRuntime: "crates/modules/rustok-product/src/runtime.rs",
  identities: "crates/modules/rustok-product/src/variant_identity_read_port.rs",
  service: "crates/modules/rustok-pricing/src/services/pricing.rs",
  ports: "crates/modules/rustok-pricing/src/ports.rs",
  read: "crates/modules/rustok-pricing/src/read_context.rs",
  write: "crates/modules/rustok-pricing/src/write_context.rs",
  publicError: "crates/modules/rustok-pricing/src/public_error.rs",
  http: "crates/modules/rustok-commerce/src/controllers/mod.rs",
  restCart: "crates/modules/rustok-commerce/src/controllers/store/carts.rs",
  restReprice: "crates/modules/rustok-commerce/src/controllers/store/mod.rs",
  query: "crates/modules/rustok-commerce/src/graphql/query.rs",
  queryShim:
    "crates/modules/rustok-commerce/src/graphql/safe_query/source/rustok_pricing_shim.rs",
  mutations: "crates/modules/rustok-commerce/src/graphql/mutations/cart.rs",
  cartShim: "crates/modules/rustok-commerce/src/graphql/mutations/safe_cart.rs",
  checkoutRuntime:
    "crates/modules/rustok-commerce/src/storefront_checkout_runtime.rs",
  staged:
    "crates/modules/rustok-commerce/src/storefront_staged_checkout_runtime.rs",
  resolver: "crates/modules/rustok-commerce/src/storefront_checkout_pricing.rs",
  typedReprice:
    "crates/modules/rustok-commerce/src/graphql/mutations/typed_reprice_helper.rs",
  cartErrors:
    "crates/modules/rustok-commerce/src/graphql/mutations/safe_helpers.rs",
  mountedRuntime:
    "crates/modules/rustok-commerce/src/storefront_checkout_runtime_mounted.rs",
  nativeBuyer:
    "crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs",
  nativeCart:
    "crates/modules/rustok-cart/storefront/src/transport/buyer_native.rs",
  nativeDecrement:
    "crates/modules/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
  nativePricing:
    "crates/modules/rustok-pricing/storefront/src/transport/native_server_adapter.rs",
  nativeAdmin:
    "crates/modules/rustok-pricing/admin/src/transport/native_server_adapter.rs",
  nativeOrder:
    "crates/modules/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs",
  nativePayment:
    "crates/modules/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs",
  nativeShipping:
    "crates/modules/rustok-fulfillment/storefront/src/transport/native_server_adapter/server_functions.rs",
};

const compact = (text) => text.replace(/\s+/g, "").replace(/,(?=\))/g, "");

// Isolate the actual function body, ignoring braces in strings and comments.
function functionBody(source, name) {
  const pattern = new RegExp(`\\bfn\\s+${name}\\s*(?:<[^>]*>)?\\s*\\(`);
  const match = pattern.exec(source);
  if (!match) return "";
  const start = source.indexOf("{", match.index + match[0].length);
  let depth = 0;
  let quoted = false;
  let escaped = false;
  let lineComment = false;
  let blockComment = 0;
  for (let index = start; index >= 0 && index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (lineComment) {
      if (character === "\n") lineComment = false;
      continue;
    }
    if (blockComment) {
      if (character === "/" && next === "*") {
        blockComment += 1;
        index += 1;
      } else if (character === "*" && next === "/") {
        blockComment -= 1;
        index += 1;
      }
      continue;
    }
    if (quoted) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') quoted = false;
      continue;
    }
    if (character === "/" && next === "/") {
      lineComment = true;
      index += 1;
      continue;
    }
    if (character === "/" && next === "*") {
      blockComment = 1;
      index += 1;
      continue;
    }
    if (character === '"') {
      quoted = true;
      continue;
    }
    if (character === "{") depth += 1;
    else if (character === "}" && --depth === 0)
      return source.slice(start, index + 1);
  }
  return "";
}

export function verifyPricingProductIdentityContext(sources) {
  const failures = [];
  for (const name of Object.keys(sourcePaths)) {
    if (typeof sources[name] !== "string")
      failures.push(`missing source: ${name}`);
  }
  if (failures.length) return failures;
  const require = (source, marker, label) => {
    if (!compact(source).includes(compact(marker)))
      failures.push(`${label}: missing ${marker}`);
  };
  const forbid = (source, marker, label) => {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  };
  const before = (source, first, second, label) => {
    const left = source.indexOf(first);
    const right = source.indexOf(second);
    if (left < 0 || right < 0 || left >= right)
      failures.push(`${label}: admission must precede the dependent read`);
  };

  require(sources.productRuntime, "variant_identity_read_port: Option<Arc<dyn ProductVariantIdentityReadPort>>", "explicit Product capability");
  require(functionBody(
    sources.productRuntime,
    "in_process",
  ), ".with_variant_identity_read_port(catalog)", "shared embedded Product owner");
  require(functionBody(
    sources.productRuntime,
    "external",
  ), "Self::new(read_port, ProductCatalogReadProfile::External)", "external selection");
  require(sources.identities, "pub tenant_id: Uuid", "required response tenant scope");
  for (const marker of [
    "response.len() != request.variant_ids.len()",
    "identity.tenant_id != tenant_id",
    "identity.variant_id != *expected",
    "identity.product_id != id",
  ]) {
    require(functionBody(
      sources.identities,
      "validate_variant_identities_response",
    ), marker, "exact owner response correlation");
  }
  for (const marker of [
    "products.contains(&identity.product_id)",
    "seen.insert(identity.variant_id)",
    "*count > MAX_VARIANT_IDENTITY_BATCH",
  ]) {
    require(functionBody(
      sources.identities,
      "validate_product_variant_identity_list_response",
    ), marker, "bounded page identity correlation");
  }

  const identity = functionBody(
    sources.service,
    "read_product_variant_identities",
  );
  require(identity, ".read_variant_identities(context.clone(), request.clone())", "original delegated identity context");
  require(identity, "validate_variant_identities_response(context, request, &identities)", "verified exact response");
  require(identity, ".map_err(pricing_identity_port_error)", "static Pricing identity dependency envelope");
  const getter = functionBody(sources.service, "variant_identity_read_port");
  require(getter, "self.product_catalog_read_runtime .variant_identity_read_port() .ok_or_else", "selected capability only");
  require(getter, '"pricing.product_identity_capability_unavailable"', "missing capability is an explicit refusal");
  forbid(
    sources.service,
    "CatalogService::new",
    "Pricing must not reconstruct Product catalog reads",
  );
  forbid(getter, "in_process", "no missing-capability local fallback");
  const quote = functionBody(
    sources.ports,
    "resolve_product_prices_in_context",
  );
  before(
    quote,
    "read_product_variant_identities",
    "resolve_price_snapshot",
    "SKU proof before price rows",
  );
  const page = functionBody(sources.service, "list_storefront_product_pricing");
  require(page, "self .product_catalog_read_runtime .read_port() .list_legacy_storefront_products", "selected inspection catalog");
  require(page, "identity_port .list_product_variant_identities(context.clone(), identity_request.clone())", "selected page identities");
  before(
    page,
    "validate_product_variant_identity_list_response",
    "entities::price::Entity::find",
    "page proof before price rows",
  );
  require(page, ".map_err(pricing_catalog_port_error)", "static catalog dependency envelope");
  require(page, ".map_err(pricing_identity_port_error)", "static page identity dependency envelope");
  require(page, "resolution::validate_row(&price, &price.currency_code, None)", "one canonical stored-money validator for inspection");
  const decrement = functionBody(
    sources.nativeDecrement,
    "storefront_cart_decrement_line_item",
  );
  require(decrement, "rustok_pricing::in_process_pricing_read_port( scope.host.db_clone(), events.clone(), product", "native decrement selected Pricing facade");
  require(decrement, ".update_line_item_quantity(", "native decrement shared quantity workflow");
  require(decrement, "super::buyer_native::port_context(&scope,", "native decrement actual caller context");
  require(functionBody(
    sources.nativeCart,
    "port_context",
  ), "scope.request.correlation_id.clone()", "native caller correlation");
  require(functionBody(
    sources.nativeCart,
    "port_context",
  ), "scope.request.locale.clone()", "native string caller locale");
  forbid(
    decrement,
    "cart.locale_code.clone()",
    "optional Cart locale must not enter a string constructor",
  );
  forbid(
    decrement,
    "PricingService::new",
    "no native decrement embedded substitution",
  );
  forbid(decrement, '"en"', "no fabricated decrement locale");
  const cartInput = functionBody(sources.resolver, "build_cart_quote_input");
  const cartScope = functionBody(sources.resolver, "cart_pricing_context");
  for (const [marker, label] of [
    [
      "Uuid::parse_str(&context.tenant_id).ok() != Some(cart.tenant_id)",
      "Cart quote tenant binding",
    ],
    ["line.cart_id != cart.id", "Cart quote line binding"],
    [
      "cart.locale_code.as_deref().unwrap_or(&context.locale)",
      "saved-or-caller Cart locale",
    ],
    [
      "context.locale = normalize_locale_tag(locale)",
      "canonical Cart locale admission",
    ],
    [
      "context.channel = cart.channel_slug.clone()",
      "Cart channel replaces page context",
    ],
    ["channel_id: cart.channel_id", "saved Cart channel ID only"],
    ["channel_slug: cart.channel_slug.clone()", "saved Cart channel slug only"],
    ["price_list_id: None", "no explicit preview handoff to Cart"],
  ])
    require([
      "saved-or-caller Cart locale",
      "canonical Cart locale admission",
      "Cart channel replaces page context",
    ].includes(label)
      ? cartScope
      : cartInput, marker, label);
  forbid(cartInput, "PortContext::new", "Cart quote retains caller metadata");
  forbid(
    cartInput,
    ".or(request_context",
    "Cart quote must not borrow page scope",
  );
  const cartRead = functionBody(
    sources.resolver,
    "read_storefront_cart_line_quote",
  );
  before(
    cartRead,
    "build_cart_quote_input",
    ".resolve_product_price",
    "Cart admission before quote IO",
  );
  require(cartRead, "cart_quote_matches(&request, &quote)", "all repricers correlate owner quote before writing");
  require(functionBody(
    sources.resolver,
    "checkout_resolution_error",
  ), "public.message, error.retryable", "checkout owner retryability");
  require(sources.resolver, "self.context.clone()", "staged Pricing keeps captured caller context");
  require(sources.staged, "cart_port_context.clone()", "staged Pricing captures admitted Cart context");
  forbid(
    sources.resolver,
    "checkout_pricing_port_context",
    "no synthetic staged caller reconstruction",
  );
  require(functionBody(
    sources.staged,
    "map_owner_port_error",
  ), "PortErrorKind::Unavailable | PortErrorKind::Timeout if error.retryable", "staged permanent refusals are not transient");
  require(functionBody(
    sources.staged,
    "map_checkout_error",
  ), "crate::CheckoutError::BoundaryFailure { retryable: true, .. }", "final staged boundary keeps retryable Cart pricing failures");
  require(sources.mountedRuntime, "error.owner_retryable().unwrap_or(policy.retryable)", "mounted runtime keeps typed owner retryability");
  require(functionBody(
    sources.cartErrors,
    "cart_port_error",
  ), "let retryable = error.retryable", "GraphQL Cart retains Pricing retryability");
  for (const name of ["restReprice", "typedReprice"]) {
    require(sources[
      name
    ], "crate::read_storefront_cart_line_quote(", `${name}: shared saved-Cart pricing reader`);
  }
  require(sources.checkoutRuntime, "crate::prepare_storefront_cart_context(", "native context/payment preparation uses selected batch quotes before its single Cart write");
  for (const name of ["restCart", "mutations"]) {
    require(sources[
      name
    ], ".update_line_item_quantity(", `${name}: shared quantity observation workflow`);
  }
  require(sources.typedReprice, "super::cart::contextual_pricing_read_port( db.clone(), event_bus.clone(), crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(", "mounted GraphQL alias receives selected runtime as its third argument");
  require(sources.typedReprice, "public_graphql_error(failure.error.retryable)", "typed GraphQL repricing keeps owner retryability");
  const admin = functionBody(
    sources.service,
    "get_admin_product_pricing_with_locale_fallback",
  );
  before(
    admin,
    "validate_pricing_product_projection",
    "entities::price::Entity::find",
    "admin inspection proof before price rows",
  );
  require(admin, ".read_product_projection( context.clone()", "admin caller context");

  for (const [kind, wrapper] of [
    ["read", sources.read],
    ["write", sources.write],
  ]) {
    require(wrapper, "product_runtime: rustok_product::ProductCatalogReadRuntime", `mandatory ${kind} composition dependency`);
    require(wrapper, ".with_product_catalog_read_runtime(product_runtime)", `selected ${kind} service dependency`);
    require(functionBody(
      sources.ports,
      `in_process_pricing_${kind}_port`,
    ), `crate::${kind}_context::in_process_pricing_${kind}_port(db, event_bus, product_runtime)`, `canonical module-path ${kind} facade`);
  }
  const budgets =
    sources.read.match(/crate::ports::read_with_deadline\(/g) ?? [];
  if (budgets.length !== 8)
    failures.push(
      "all eight canonical Pricing read operations must retain a caller wait budget",
    );
  require(functionBody(
    sources.service,
    "require_embedded_variant_state",
  ), '"pricing.product_write_capability_unavailable"', "no fabricated external SKU writer");
  require(sources.publicError, "retryable: owner_retryable", "owner retryability is not reinterpreted");

  for (const [source, marker, label] of [
    [
      sources.http,
      "self.product_catalog_read_runtime.clone()",
      "HTTP selected runtime getter",
    ],
    [
      sources.restCart,
      "runtime.product_catalog_read_runtime()",
      "REST Cart composition",
    ],
    [
      sources.restReprice,
      "in_process_pricing_read_port(db.clone(), event_bus, product_runtime)",
      "REST context repricing",
    ],
    [
      sources.query,
      "crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(",
      "GraphQL queries",
    ],
    [
      sources.mutations,
      "crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(",
      "GraphQL Cart mutations",
    ],
    [
      sources.queryShim,
      "::rustok_pricing::in_process_pricing_read_port(db, event_bus, product_runtime)",
      "GraphQL query facade",
    ],
    [
      sources.cartShim,
      "::rustok_pricing::in_process_pricing_read_port(db, event_bus, product_runtime)",
      "GraphQL Cart facade",
    ],
    [
      sources.checkoutRuntime,
      "in_process_pricing_read_port( runtime.db_clone(), runtime.event_bus(), runtime.product_catalog_read_runtime()",
      "checkout preparation repricing",
    ],
    [
      sources.resolver,
      "in_process_pricing_read_port( self.db.clone(), self.event_bus.clone(), self.product_catalog_read_runtime.clone()",
      "durable checkout repricing",
    ],
    [
      sources.staged,
      "product_catalog_read_runtime.clone()",
      "staged resolver capability capture",
    ],
    [
      sources.staged,
      "product_catalog_read_runtime.read_port()",
      "same staged catalog owner",
    ],
    [
      sources.nativeBuyer,
      "product_read_runtime(&scope)",
      "Product native buyer quote",
    ],
    [
      sources.nativeCart,
      "in_process_pricing_read_port(scope.host.db_clone(), events.clone(), product)",
      "Cart native add",
    ],
    [
      sources.nativePricing,
      "shared_get::<rustok_product::ProductCatalogReadRuntime>()",
      "Pricing native atlas",
    ],
    [
      sources.nativeAdmin,
      ".with_product_catalog_read_runtime(product_runtime.clone())",
      "one native admin metadata/quote selection",
    ],
    [
      sources.nativeAdmin,
      "pricing_admin_read_context(",
      "original native admin caller",
    ],
    [
      sources.nativeOrder,
      "product_catalog_read_runtime",
      "native staged completion",
    ],
    [
      sources.nativePayment,
      "runtime.with_product_catalog_read_runtime(product)",
      "native payment preparation",
    ],
    [
      sources.nativeShipping,
      "runtime.with_product_catalog_read_runtime(product)",
      "native shipping repricing",
    ],
  ])
    require(source, marker, label);

  const implicitRead =
    /in_process_pricing_read_port\(\s*(?:db\.clone\(\)|runtime\.db_clone\(\)),\s*(?:event_bus(?:\.clone\(\))?|runtime\.event_bus\(\))\s*\)/;
  for (const name of [
    "restCart",
    "restReprice",
    "query",
    "mutations",
    "checkoutRuntime",
    "resolver",
    "nativeBuyer",
    "nativeCart",
    "nativeDecrement",
    "nativePricing",
  ]) {
    if (implicitRead.test(sources[name]))
      failures.push(
        `${name}: implicit embedded Pricing factory bypasses the selected Product runtime`,
      );
  }
  return failures;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const root =
    process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() ||
    fileURLToPath(new URL("../../", import.meta.url));
  const sources = Object.fromEntries(
    Object.entries(sourcePaths).map(([name, relative]) => [
      name,
      readFileSync(path.join(root, relative), "utf8"),
    ]),
  );
  const failures = verifyPricingProductIdentityContext(sources);
  if (failures.length) {
    console.error(
      "Selected Product/Pricing identity source verification failed:\n" +
        failures.map((failure) => `✗ ${failure}`).join("\n"),
    );
    process.exitCode = 1;
  } else {
    console.log(
      "✔ Selected Product capabilities, tenant/SKU response correlation, bounded Pricing facades and actual consumer composition retain one money engine; source only, not execution evidence",
    );
  }
}
