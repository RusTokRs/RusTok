import assert from 'node:assert/strict';
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repository = fileURLToPath(new URL('../../', import.meta.url));
const verifier = fileURLToPath(
  new URL(
    './verify-commerce-checkout-plan-product-projection-context.mjs',
    import.meta.url,
  ),
);
const paths = {
  cart: 'crates/modules/rustok-commerce/src/storefront_cart_runtime.rs',
  plan: 'crates/modules/rustok-commerce/src/services/checkout_plan_builder.rs',
  catalog: 'crates/modules/rustok-commerce/src/services/checkout_catalog.rs',
  direct: 'crates/modules/rustok-commerce/src/services/checkout.rs',
};

function verify(mutate = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'checkout-sku-contract-'));
  try {
    const sources = Object.fromEntries(
      Object.entries(paths).map(([name, relative]) => [
        name,
        readFileSync(path.join(repository, relative), 'utf8'),
      ]),
    );
    mutate(sources);
    for (const [name, relative] of Object.entries(paths)) {
      const destination = path.join(root, relative);
      mkdirSync(path.dirname(destination), { recursive: true });
      writeFileSync(destination, sources[name]);
    }
    const result = spawnSync(process.execPath, [verifier], {
      encoding: 'utf8',
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    });
    assert.ifError(result.error);
    return result;
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

// Source fixtures only: these do not establish Rust, database or mounted behavior.
test('the real shared owner calls and UUID preconditions satisfy the source guard', () => {
  const result = verify();
  assert.equal(result.status, 0, result.stderr);
});

test('a per-line or constant stock probe cannot replace the aggregate SKU demand', () => {
  const result = verify((sources) => {
    sources.plan = sources.plan.replace(
      'requested_quantity: demand.quantity',
      'requested_quantity: 1',
    );
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /combined inventory demand/);
});

test('the shared helper must select the exact requested UUID, not any available SKU', () => {
  const result = verify((sources) => {
    sources.catalog = sources.catalog.replace(
      '.filter(|variant| variant.id == variant_id)',
      '.filter(|_| true)',
    );
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /exact selected UUID membership/);
});

test('direct checkout cannot bypass the shared Cart admission', () => {
  const result = verify((sources) => {
    sources.direct = sources.direct.replaceAll(
      'super::checkout_catalog::variant_demands(cart)?',
      'unchecked_demands(cart)?',
    );
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /direct demand admission/);
});

test('guest delegation cannot be represented by a fabricated User actor', () => {
  const result = verify((sources) => {
    sources.catalog = sources.catalog.replace(
      'rustok_api::PortActor::service("rustok-commerce.checkout")',
      'rustok_api::PortActor::user(actor_id.to_string())',
    );
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /truthful guest delegation/);
});

test('retained owner context must be cloned into both Product branches', () => {
  const result = verify((sources) => {
    sources.plan = sources.plan.replace(
      'product_context.clone()',
      'unrelated_context.clone()',
    );
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /two branch clones/);
});

test('Cart must call Inventory rather than use Product descriptive stock', () => {
  const result = verify((sources) => {
    sources.cart = sources.cart.replace(
      '.check_availability(context, request)',
      '.use_product_projection(context, request)',
    );
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Cart stock owner call/);
});
