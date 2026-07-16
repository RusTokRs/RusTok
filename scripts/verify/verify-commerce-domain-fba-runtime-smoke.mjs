import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const defaultRoot = process.env.COMMERCE_DOMAIN_FBA_ROOT || process.cwd();
export const commerceDomainModules = ['product', 'pricing', 'inventory', 'customer', 'cart', 'tax'];
const invocationTracePath = 'crates/rustok-commerce/contracts/evidence/commerce-domain-provider-invocation-trace.json';

export class CommerceDomainFbaRuntimeSmokeError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CommerceDomainFbaRuntimeSmokeError';
  }
}

const fail = (message) => { throw new CommerceDomainFbaRuntimeSmokeError(message); };
const sameSet = (actual, expected) =>
  Array.isArray(actual) && Array.isArray(expected) &&
  actual.length === expected.length && expected.every((item) => actual.includes(item));

function functionBody(source, name) {
  const signature = new RegExp(`async\\s+fn\\s+${name}\\s*\\(`, 'g');
  let match;
  while ((match = signature.exec(source)) !== null) {
    const semicolon = source.indexOf(';', match.index);
    const open = source.indexOf('{', match.index);
    if (open < 0 || (semicolon >= 0 && semicolon < open)) continue;
    let depth = 0;
    for (let index = open; index < source.length; index += 1) {
      if (source[index] === '{') depth += 1;
      if (source[index] === '}' && --depth === 0) return source.slice(open + 1, index);
    }
  }
  return null;
}

function simulatePolicy({ deadlineMs, write, idempotencyKey }) {
  if (!deadlineMs || deadlineMs <= 0) return { ok: false, code: 'port.deadline_required' };
  if (write && !idempotencyKey) return { ok: false, code: 'port.idempotency_key_required' };
  return { ok: true };
}

export function verifyCommerceDomainFbaRuntimeSmoke({ root = defaultRoot, modules = commerceDomainModules } = {}) {
  const read = (repoPath) => fs.readFileSync(path.join(root, repoPath), 'utf8');
  const json = (repoPath) => JSON.parse(read(repoPath));
  const trace = json(invocationTracePath);
  const commerceRegistry = json('crates/rustok-commerce/contracts/commerce-fba-registry.json');
  const commerceFbaSource = read('crates/rustok-commerce/src/fba.rs');
  const checkoutSource = read('crates/rustok-commerce/src/services/checkout.rs');
  const cartServiceSource = read('crates/rustok-cart/src/services/cart.rs');
  const cartHelpersSource = read('crates/rustok-cart/src/services/cart/helpers.rs');
  const cartErrorSource = read('crates/rustok-cart/src/error.rs');
  const cartStorefrontRestSource = read('crates/rustok-commerce/src/controllers/store/carts.rs');
  const cartStorefrontGraphqlSource = read('crates/rustok-commerce/src/graphql/query.rs');
  const cartStorefrontMutationSource = read('crates/rustok-commerce/src/graphql/mutations/cart.rs');
  const cartCheckoutRestSource = read('crates/rustok-commerce/src/controllers/store/checkout.rs');
  const cartCheckoutGraphqlSource = read('crates/rustok-commerce/src/graphql/mutations/checkout.rs');
  const cartCheckoutRuntimeSource = read('crates/rustok-commerce/src/storefront_checkout_runtime.rs');
  const cartShippingOptionsSource = read('crates/rustok-commerce/src/controllers/store/products.rs');
  const cartPromotionMutationSource = read('crates/rustok-commerce/src/graphql/mutations/pricing.rs');
  const cartPromotionNativeAdapterSource = read('crates/rustok-commerce/admin/src/transport/native_server_adapter.rs')
    .split('#[cfg(all(test, feature = "ssr"))]')[0];
  const checkoutPricingSource = read('crates/rustok-commerce/src/storefront_checkout_pricing.rs');
  const graphqlMutationHelpersSource = read('crates/rustok-commerce/src/graphql/mutations/helpers.rs');
  const pricingQuerySource = read('crates/rustok-commerce/src/graphql/query.rs');
  const storefrontControllerSource = read('crates/rustok-commerce/src/controllers/store/mod.rs');
  const storefrontCheckoutRuntimeSource = read('crates/rustok-commerce/src/storefront_checkout_runtime.rs');

  if (trace.schema_version !== 1) fail('commerce-domain invocation trace schema_version drift');
  if (trace.status !== 'executable_no_compile') fail('commerce-domain invocation trace status drift');
  if (trace.runner !== 'scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs') {
    fail('commerce-domain invocation trace runner drift');
  }
  if (trace.generated_from !== 'crates/rustok-commerce/contracts/commerce-fba-registry.json') {
    fail('commerce-domain invocation trace source drift');
  }
  if (commerceRegistry.evidence?.runtime_invocation_trace !== invocationTracePath) {
    fail('commerce registry runtime invocation trace evidence drift');
  }
  if (!commerceFbaSource.includes('COMMERCE_DOMAIN_PROVIDER_INVOCATION_TRACE_JSON')) {
    fail('commerce fba.rs must expose the invocation trace as a typed runtime entrypoint');
  }
  if (!commerceFbaSource.includes('include_str!("../contracts/evidence/commerce-domain-provider-invocation-trace.json")')) {
    fail('commerce fba.rs must embed the invocation trace artifact');
  }
  if (!commerceFbaSource.includes('pub fn commerce_domain_provider_invocation_trace')) {
    fail('commerce fba.rs must publish an invocation trace parser');
  }
  for (const marker of [
    'impl CommerceFbaRegistry',
    'pub fn provider(&self, module: &str)',
    'pub fn provider_modules(&self)',
    'impl CommerceDomainProviderInvocationTrace',
    'pub fn provider_entry(',
    'pub fn consumer_entries(',
  ]) {
    if (!commerceFbaSource.includes(marker)) {
      fail(`commerce fba.rs missing typed lookup helper: ${marker}`);
    }
  }
  if (!sameSet(trace.modules.map((entry) => entry.provider_module), modules)) {
    fail('commerce-domain invocation trace module set drift');
  }

  for (const marker of [
    'inventory_reservation_port: Arc<dyn InventoryReservationPort>',
    '.inventory_reservation_port',
    '.check_availability(',
    'InventoryAvailabilityRequest {',
    'checkout_inventory_port_context(',
    'PortActor::user(actor_id.to_string())',
    '.with_deadline(Duration::from_secs(2))',
    'checkout_port_error("check_inventory_availability", error)',
    'CheckoutError::BoundaryFailure',
  ]) {
    if (!checkoutSource.includes(marker)) {
      fail(`checkout inventory provider-consumer boundary missing: ${marker}`);
    }
  }
  if (checkoutSource.includes('check_variant_availability_for_public_channel')) {
    fail('checkout must not bypass InventoryReservationPort through the public inventory helper');
  }
  for (const marker of [
    'product_catalog_read_port: Arc<dyn ProductCatalogReadPort>',
    '.product_catalog_read_port',
    '.read_product_projection(',
    '.read_variant_product_projection(',
    'ProductProjectionRequest {',
    'VariantProductProjectionRequest {',
    'checkout_product_port_context(',
    'checkout_port_error("read_checkout_product_projection", error)',
  ]) {
    if (!checkoutSource.includes(marker)) {
      fail(`checkout product provider-consumer boundary missing: ${marker}`);
    }
  }
  if (checkoutSource.includes('rustok_product::entities')) {
    fail('checkout must not import product entities outside the ProductCatalogReadPort boundary');
  }
  for (const marker of [
    'cart_checkout_port: Arc<dyn CartCheckoutPort>',
    '.read_cart_checkout_snapshot(',
    '.update_cart_checkout_context(',
    '.begin_cart_checkout(',
    '.release_cart_checkout(',
    '.complete_cart_checkout(',
    'checkout_cart_port_context(',
    'CartCheckoutLifecycleRequest {',
    'write: bool',
    'context.with_idempotency_key(',
  ]) {
    if (!checkoutSource.includes(marker)) {
      fail(`checkout cart provider-consumer boundary missing: ${marker}`);
    }
  }
  if (checkoutSource.includes('CartService::new(')) {
    fail('checkout must not construct CartService outside the CartCheckoutPort boundary');
  }

  for (const marker of [
    'tax_calculation_port: Arc<dyn TaxCalculationPort>',
    'in_process_tax_calculation_port()',
    'with_tax_calculation_port',
  ]) {
    if (!cartServiceSource.includes(marker)) {
      fail(`cart tax provider-consumer boundary missing: ${marker}`);
    }
  }
  for (const marker of [
    '.calculate_tax(',
    'cart_tax_port_context(cart)',
    'PortActor::service("rustok-cart.tax")',
    '.with_deadline(Duration::from_secs(2))',
    'CartError::TaxBoundary',
  ]) {
    if (!cartHelpersSource.includes(marker)) {
      fail(`cart tax provider-consumer invocation missing: ${marker}`);
    }
  }
  if (!cartErrorSource.includes('TaxBoundary')) {
    fail('cart tax provider-consumer boundary must preserve typed port errors');
  }
  if (cartServiceSource.includes('TaxService') || cartHelpersSource.includes('TaxService')) {
    fail('cart must not access TaxService outside the TaxCalculationPort boundary');
  }
  for (const [surface, source] of [
    ['REST storefront cart adapter', cartStorefrontRestSource],
    ['GraphQL storefront cart query adapter', cartStorefrontGraphqlSource],
    ['GraphQL storefront cart mutation adapter', cartStorefrontMutationSource],
    ['REST storefront checkout adapter', cartCheckoutRestSource],
    ['GraphQL storefront checkout adapter', cartCheckoutGraphqlSource],
    ['Storefront checkout runtime adapter', cartCheckoutRuntimeSource],
    ['REST storefront shipping-options adapter', cartShippingOptionsSource],
  ]) {
    for (const marker of [
      'in_process_cart_storefront_port(',
      '.read_storefront_cart(',
      'CartStorefrontReadRequest',
    ]) {
      if (!source.includes(marker)) {
        fail(`${surface} missing CartStorefrontPort consumer marker: ${marker}`);
      }
    }
    if (source.includes('CartService::new(')) {
      fail(`${surface} must not construct CartService outside CartStorefrontPort`);
    }
  }
  for (const marker of [
    '.create_storefront_cart(',
    '.add_storefront_line_item(',
    '.update_storefront_context(',
    '.update_storefront_line_item_quantity(',
    '.update_storefront_line_item_pricing(',
    '.remove_storefront_line_item(',
  ]) {
    if (!cartStorefrontMutationSource.includes(marker)) {
      fail(`GraphQL storefront cart mutation adapter missing CartStorefrontPort operation: ${marker}`);
    }
  }
  for (const marker of [
    'in_process_cart_promotion_port(',
    '.read_cart_promotion_preview(',
    '.apply_cart_promotion(',
    'CartPromotionRequest',
  ]) {
    if (!cartPromotionMutationSource.includes(marker)) {
      fail(`GraphQL cart promotion adapter missing CartPromotionPort operation: ${marker}`);
    }
  }
  if (cartPromotionMutationSource.includes('CartService::new(')) {
    fail('GraphQL cart promotion adapter must not construct CartService outside CartPromotionPort');
  }
  for (const [functionName, operation, request] of [
    ['update_admin_pricing_variant_price', 'upsert_variant_price', 'UpsertVariantPriceRequest'],
    ['apply_admin_pricing_variant_discount', 'apply_variant_discount', 'ApplyVariantDiscountRequest'],
    ['update_admin_pricing_price_list_rule', 'set_price_list_percentage_rule', 'SetPriceListPercentageRuleRequest'],
    ['update_admin_pricing_price_list_scope', 'set_price_list_scope', 'SetPriceListScopeRequest'],
  ]) {
    const body = functionBody(cartPromotionMutationSource, functionName);
    if (!body) fail(`GraphQL pricing mutation implementation body missing: ${functionName}`);
    for (const marker of ['in_process_pricing_write_port(', `.${operation}(`, request, 'pricing_write_port_context(']) {
      if (!body.includes(marker)) {
        fail(`GraphQL pricing mutation ${functionName} missing PricingWritePort marker: ${marker}`);
      }
    }
    if (body.includes('PricingService::new(')) {
      fail(`GraphQL pricing mutation ${functionName} must not construct PricingService outside PricingWritePort`);
    }
  }
  for (const marker of [
    'in_process_cart_promotion_port(',
    '.read_cart_promotion_preview(',
    '.apply_cart_promotion(',
    'CartPromotionRequest',
  ]) {
    if (!cartPromotionNativeAdapterSource.includes(marker)) {
      fail(`Native cart promotion adapter missing CartPromotionPort operation: ${marker}`);
    }
  }
  if (cartPromotionNativeAdapterSource.includes('CartService::new(')) {
    fail('Native cart promotion adapter must not construct CartService outside CartPromotionPort');
  }
  for (const marker of [
    'in_process_pricing_read_port(',
    '.resolve_product_price(',
    'ResolveProductPriceRequest {',
    'checkout_pricing_port_context(',
    'PortActor::service("rustok-commerce.checkout-pricing")',
    '.with_deadline(std::time::Duration::from_secs(2))',
  ]) {
    if (!checkoutPricingSource.includes(marker)) {
      fail(`checkout pricing provider-consumer boundary missing: ${marker}`);
    }
  }
  if (checkoutPricingSource.includes('PricingService::new(')) {
    fail('checkout pricing must not construct PricingService outside PricingReadPort');
  }
  const effectivePriceBody = functionBody(pricingQuerySource, 'attach_effective_prices');
  if (!effectivePriceBody) fail('GraphQL effective-price attachment implementation body missing');
  for (const marker of [
    '.resolve_product_price(',
    'ResolveProductPriceRequest {',
    'pricing_query_port_context(',
    'PortErrorKind::NotFound',
  ]) {
    if (!effectivePriceBody.includes(marker)) {
      fail(`GraphQL effective-price attachment missing PricingReadPort marker: ${marker}`);
    }
  }
  if (effectivePriceBody.includes('PricingService::new(') || effectivePriceBody.includes('.resolve_variant_price(')) {
    fail('GraphQL effective-price attachment must not bypass PricingReadPort');
  }
  for (const functionName of ['admin_pricing_product', 'storefront_pricing_product']) {
    const body = functionBody(pricingQuerySource, functionName);
    if (!body) fail(`GraphQL pricing query implementation body missing: ${functionName}`);
    for (const marker of ['in_process_pricing_read_port(', 'attach_effective_prices(']) {
      if (!body.includes(marker)) {
        fail(`GraphQL pricing query ${functionName} missing PricingReadPort composition marker: ${marker}`);
      }
    }
    if (body.includes('PricingService::new(')) {
      fail(`GraphQL pricing query ${functionName} must not construct PricingService outside PricingReadPort`);
    }
  }
  const activePriceListsBody = functionBody(pricingQuerySource, 'storefront_active_price_lists');
  if (!activePriceListsBody) fail('GraphQL active-price-list query implementation body missing');
  for (const marker of [
    'in_process_pricing_read_port(',
    '.list_active_price_list_projections(',
    'ActivePriceListProjectionRequest {',
    'pricing_active_lists_port_context(',
  ]) {
    if (!activePriceListsBody.includes(marker)) {
      fail(`GraphQL active-price-list query missing PricingReadPort marker: ${marker}`);
    }
  }
  if (activePriceListsBody.includes('PricingService::new(') || activePriceListsBody.includes('.list_active_price_lists_for_channel(')) {
    fail('GraphQL active-price-list query must not bypass PricingReadPort');
  }
  for (const [surface, source, functionName, providerSource, providerFunctionName] of [
    ['GraphQL storefront repricing helper', graphqlMutationHelpersSource, 'reprice_storefront_cart_line_items'],
    [
      'GraphQL storefront add-line-item helper',
      graphqlMutationHelpersSource,
      'resolve_storefront_line_item_input',
      cartStorefrontMutationSource,
      'add_storefront_cart_line_item',
    ],
    [
      'GraphQL storefront update-line-item mutation',
      cartStorefrontMutationSource,
      'update_storefront_cart_line_item',
    ],
    ['REST storefront repricing helper', storefrontControllerSource, 'reprice_storefront_cart_line_items_for_db'],
    [
      'REST storefront add-line-item helper',
      storefrontControllerSource,
      'resolve_store_line_item_input',
      cartStorefrontRestSource,
      'add_cart_line_item',
    ],
    ['Storefront checkout runtime repricing helper', storefrontCheckoutRuntimeSource, 'reprice_storefront_cart_line_items'],
  ]) {
    const body = functionBody(source, functionName);
    if (!body) fail(`${surface} implementation body missing`);
    const providerBody = providerFunctionName
      ? functionBody(providerSource, providerFunctionName)
      : body;
    if (!providerBody) fail(`${surface} provider implementation body missing`);
    if (!providerBody.includes('in_process_pricing_read_port(')) {
      fail(`${surface} missing PricingReadPort provider factory`);
    }
    for (const marker of [
      '.resolve_product_price(',
      'ResolveProductPriceRequest {',
    ]) {
      if (!body.includes(marker)) {
        fail(`${surface} missing PricingReadPort consumer marker: ${marker}`);
      }
    }
    if (body.includes('PricingService::new(') || body.includes('.resolve_variant_price(')) {
      fail(`${surface} must not bypass PricingReadPort`);
    }
  }

  for (const module of modules) {
    const registryPath = `crates/rustok-${module}/contracts/${module}-fba-registry.json`;
    const smokePath = `crates/rustok-${module}/contracts/evidence/${module}-runtime-contract-smoke.json`;
    const registry = json(registryPath);
    const smoke = json(smokePath);
    const runtimeFallbackSmoke = module === 'product' && registry.evidence?.runtime_fallback_smoke
      ? json(registry.evidence.runtime_fallback_smoke)
      : null;
    const ports = read(`crates/rustok-${module}/src/ports.rs`);
    const traceEntry = trace.modules.find((entry) => entry.provider_module === module);

    if (!traceEntry) fail(`${module} invocation trace entry missing`);
    if (traceEntry.provider_registry !== registryPath) fail(`${module} invocation trace provider registry drift`);
    if (traceEntry.runtime_contract_smoke !== smokePath) fail(`${module} invocation trace smoke path drift`);
    if (traceEntry.contract_version !== registry.contract_version) fail(`${module} invocation trace contract version drift`);
    if (!sameSet(traceEntry.ports, registry.ports.map((entry) => entry.name))) fail(`${module} invocation trace port drift`);
    if (!sameSet(traceEntry.operations, registry.ports.flatMap((entry) => entry.operations))) {
      fail(`${module} invocation trace operation drift`);
    }
    if (!sameSet(traceEntry.fallback_profiles, smoke.fallback_profiles)) fail(`${module} invocation trace fallback profile drift`);
    if (!sameSet(traceEntry.degraded_modes, smoke.degraded_modes)) fail(`${module} invocation trace degraded mode drift`);

    // Temporary 2026-07-13 readiness policy permits static boundary promotion
    // without treating the no-compile smoke as transport execution evidence.
    const allowedRegistryStatuses = ['in_progress', 'boundary_ready'];
    if (!allowedRegistryStatuses.includes(registry.status)) {
      fail(`${module} registry must remain ${allowedRegistryStatuses.join(' or ')} before live runtime execution`);
    }
    if (smoke.status !== 'executable_no_compile') fail(`${module} runtime smoke status drift`);
    if (smoke.generated_from !== registryPath) fail(`${module} runtime smoke source drift`);
    if (smoke.runner !== 'scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs') fail(`${module} runtime smoke runner drift`);
    if (smoke.contract_version !== registry.contract_version) fail(`${module} runtime smoke contract version drift`);
    if (registry.evidence?.runtime_contract_smoke !== smokePath) fail(`${module} registry runtime evidence path drift`);
    if (registry.evidence?.runtime_contract_smoke_runner !== smoke.runner) fail(`${module} registry runtime runner drift`);
    if (registry.contract_tests.status !== 'planned_cases_locked') fail(`${module} contract test status drift`);
    const expectedFallbackStatus = module === 'product' && runtimeFallbackSmoke
      ? 'planned_runtime_pending'
      : 'planned';
    if (registry.contract_tests.fallback_smoke.status !== expectedFallbackStatus) {
      fail(`${module} fallback smoke must remain ${expectedFallbackStatus} before live runtime execution`);
    }
    if (!sameSet(smoke.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) fail(`${module} fallback profile drift`);
    if (!sameSet(smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail(`${module} degraded mode drift`);
    if (!sameSet(traceEntry.fallback_profiles, registry.contract_tests.fallback_smoke.profiles)) {
      fail(`${module} invocation trace fallback profile does not mirror planned fallback smoke`);
    }
    if (!sameSet(traceEntry.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) {
      fail(`${module} invocation trace degraded mode does not mirror planned fallback smoke`);
    }
    if (runtimeFallbackSmoke) {
      if (runtimeFallbackSmoke.status !== 'no_compile_executable_runtime_fallback_smoke') {
        fail(`${module} runtime fallback smoke status drift`);
      }
      if (!sameSet(runtimeFallbackSmoke.profiles, registry.contract_tests.fallback_smoke.profiles)) {
        fail(`${module} runtime fallback smoke profile drift`);
      }
      for (const profile of registry.contract_tests.fallback_smoke.profiles) {
        if (!runtimeFallbackSmoke.smoke_cases.some((entry) => entry.profile === profile && entry.execution_status === 'no_compile_executable_locked')) {
          fail(`${module} runtime fallback smoke missing executable no-compile profile ${profile}`);
        }
      }
    }

    const registryConsumer = registry.consumers.find((consumer) => consumer.module === traceEntry.consumer_module);
    if (!registryConsumer) fail(`${module} invocation trace consumer ${traceEntry.consumer_module} missing from provider registry`);
    if (!registryConsumer.fallback_profiles || !sameSet(traceEntry.consumer_fallback_profiles, registryConsumer.fallback_profiles)) {
      fail(`${module} invocation trace consumer fallback profile drift`);
    }
    if (!registryConsumer.degraded_modes || !sameSet(traceEntry.consumer_degraded_modes, registryConsumer.degraded_modes)) {
      fail(`${module} invocation trace consumer degraded mode drift`);
    }

    if (traceEntry.consumer_module === 'commerce') {
      const commerceProvider = commerceRegistry.providers.find((provider) => provider.module === module);
      if (!commerceProvider) fail(`${module} invocation trace missing from commerce consumer registry`);
      if (commerceProvider.registry !== registryPath) fail(`${module} commerce registry provider path drift`);
      if (commerceProvider.contract_version !== registry.contract_version) fail(`${module} commerce registry contract version drift`);
      if (!sameSet(commerceProvider.ports, traceEntry.ports)) fail(`${module} commerce registry port drift`);
      if (!sameSet(commerceProvider.fallback_profiles, traceEntry.consumer_fallback_profiles)) {
        fail(`${module} commerce registry fallback profile drift`);
      }
      if (!sameSet(commerceProvider.degraded_modes, traceEntry.consumer_degraded_modes)) {
        fail(`${module} commerce registry degraded mode drift`);
      }
    }

    const registryCases = registry.contract_tests.cases;
    if (!sameSet(smoke.cases.map((entry) => entry.operation), registryCases.map((entry) => entry.operation))) {
      fail(`${module} runtime operation set drift`);
    }

    for (const testCase of smoke.cases) {
      const registryCase = registryCases.find((entry) => entry.operation === testCase.operation);
      if (!registryCase) fail(`${module}.${testCase.operation} missing registry case`);
      const body = functionBody(ports, testCase.operation);
      if (!body) fail(`${module}.${testCase.operation} implementation body missing`);
      let previous = -1;
      for (const marker of testCase.source_order) {
        const index = body.indexOf(marker, previous + 1);
        if (index < 0) fail(`${module}.${testCase.operation} source marker missing: ${marker}`);
        previous = index;
      }

      const write = registryCase.assertions.includes('write_idempotency_required');
      const accepted = simulatePolicy({ deadlineMs: 250, write, idempotencyKey: write ? 'smoke-key' : null });
      if (!accepted.ok) fail(`${module}.${testCase.operation} valid context rejected`);
      const noDeadline = simulatePolicy({ deadlineMs: 0, write, idempotencyKey: 'smoke-key' });
      if (noDeadline.code !== 'port.deadline_required') fail(`${module}.${testCase.operation} deadline rejection drift`);
      if (write) {
        const noIdempotency = simulatePolicy({ deadlineMs: 250, write, idempotencyKey: null });
        if (noIdempotency.code !== 'port.idempotency_key_required') fail(`${module}.${testCase.operation} idempotency rejection drift`);
      }
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyCommerceDomainFbaRuntimeSmoke();
    console.log(`commerce-domain FBA executable runtime smoke verified: ${commerceDomainModules.join(', ')}`);
  } catch (error) {
    if (error instanceof CommerceDomainFbaRuntimeSmokeError) {
      console.error(`commerce-domain FBA runtime smoke failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
