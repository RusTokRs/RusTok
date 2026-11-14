import assert from "node:assert/strict";
import test from "node:test";

import {
  buildStrictCsp,
  createCspNonce,
  isValidCspNonce,
  shouldBypassCspForNonDocumentResource,
} from "./csp.ts";

test("issues unique CSP nonces with 128 bits of random entropy", () => {
  const first = createCspNonce();
  const second = createCspNonce();

  assert.notEqual(first, second);
  assert.equal(isValidCspNonce(first), true);
  assert.equal(Buffer.from(first, "base64").byteLength, 16);
});

test("accepts only bounded nonce source values", () => {
  assert.equal(isValidCspNonce("a".repeat(8)), true);
  assert.equal(isValidCspNonce("a".repeat(129)), false);
  assert.equal(isValidCspNonce("nonce-with spaces"), false);
  assert.equal(isValidCspNonce("ab=cd123"), false);
  assert.equal(isValidCspNonce("abcdefgh==="), false);
  assert.equal(isValidCspNonce(null), false);
});

test("keeps HTML and RSC dotted routes in the document proxy", () => {
  assert.equal(
    shouldBypassCspForNonDocumentResource("/a-page.html", new Headers({ accept: "text/html" })),
    false,
  );
  assert.equal(
    shouldBypassCspForNonDocumentResource("/a-page.html", new Headers({ accept: "text/x-component" })),
    false,
  );
});

test("bypasses static resources without misclassifying extensionless routes", () => {
  assert.equal(
    shouldBypassCspForNonDocumentResource(
      "/assets/site.css",
      new Headers({ accept: "text/css,*/*;q=0.1", "sec-fetch-dest": "style" }),
    ),
    true,
  );
  assert.equal(
    shouldBypassCspForNonDocumentResource(
      "/tenant/pages",
      new Headers({ accept: "image/avif", "sec-fetch-dest": "image" }),
    ),
    true,
  );
  assert.equal(
    shouldBypassCspForNonDocumentResource("/tenant/pages", new Headers({ accept: "*/*" })),
    false,
  );
});

test("builds strict production CSP with nonce styles and no inline attributes", () => {
  const nonce = createCspNonce();
  const policy = buildStrictCsp({ nonce });

  assert.ok(policy.includes(`script-src 'self' 'nonce-${nonce}' 'strict-dynamic'`));
  assert.ok(policy.includes(`style-src 'self' 'nonce-${nonce}'`));
  assert.match(policy, /style-src-attr 'none'/);
  assert.match(policy, /script-src-attr 'none'/);
  assert.doesNotMatch(policy, /unsafe-eval|unsafe-inline/);
});

test("permits unsafe-eval only in an explicitly opted-in development policy", () => {
  const nonce = createCspNonce();
  const policy = buildStrictCsp({ nonce, isDev: true });

  assert.match(policy, /'unsafe-eval'/);
  assert.doesNotMatch(policy, /'unsafe-inline'/);
});

test("rejects invalid nonce input instead of interpolating it into a policy", () => {
  assert.throws(
    () => buildStrictCsp({ nonce: "valid-looking'; script-src *" }),
    /invalid CSP nonce/,
  );
});
