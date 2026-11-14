/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { STOREFRONT_PAGE_QUERY, readStorefrontPage } from "./api/page-query.ts";
import { scopeLandingCss } from "./components/css-scope.ts";
import { buyerHandoffHref, safeNavigationHref } from "./components/landing-bridge.ts";

/**
 * Asserts that `css` is exactly one top-level `{}` block, as a CSS parser sees it: nothing may
 * follow the block that the leading `@scope` prelude opens. Deliberately written separately
 * from the scanner in `css-scope.ts` (token-by-token, per the CSS Syntax tokenizer rules).
 */
function assertSingleScopeBlock(css, label = css) {
  assert.ok(css.startsWith("@scope ("), `not an @scope block: ${label}`);
  const stack = [];
  const closerOf = { "{": "}", "(": ")", "[": "]" };
  let closedAt = -1;
  let i = 0;
  while (i < css.length) {
    const ch = css[i];
    if (css.startsWith("/*", i)) {
      const end = css.indexOf("*/", i + 2);
      assert.ok(end >= 0, `unterminated comment: ${label}`);
      i = end + 2;
    } else if (ch === '"' || ch === "'") {
      let j = i + 1;
      while (j < css.length && css[j] !== ch && css[j] !== "\n") j += css[j] === "\\" ? 2 : 1;
      i = css[j] === ch ? j + 1 : j;
    } else if (ch === "\\") {
      i += 2;
    } else if (/^url\(\s*[^"'\s]/i.test(css.slice(i, i + 64)) && !/[\w-]/.test(css[i - 1] ?? "")) {
      let j = i + 4;
      while (j < css.length && css[j] !== ")") j += css[j] === "\\" ? 2 : 1;
      assert.ok(j < css.length, `unterminated url(): ${label}`);
      i = j + 1;
    } else {
      if (ch in closerOf) {
        stack.push(closerOf[ch]);
      } else if (ch === "}" || ch === ")" || ch === "]") {
        if (stack.length > 0 && stack[stack.length - 1] === ch) {
          stack.pop();
          if (stack.length === 0 && ch === "}" && closedAt < 0) closedAt = i;
        } else {
          assert.ok(stack.length > 0, `stray "${ch}" at top level: ${label}`);
        }
      }
      i += 1;
    }
  }
  assert.equal(stack.length, 0, `unclosed blocks: ${label}`);
  assert.equal(closedAt, css.length - 1, `content after the @scope block: ${label}`);
}

const PAGE_ID = "0b0e4f5c-6a1d-4c58-9f0e-3d2a1b4c5d6e";

/** Field names selected inside the first `name { ... }` block of a GraphQL document. */
function selectedFields(query, name) {
  const match = new RegExp(`\\b${name}(?:\\([^)]*\\))?\\s*\\{`).exec(query);
  assert.ok(match, `the query does not select ${name}`);
  let depth = 1;
  let i = match.index + match[0].length;
  let own = "";
  while (i < query.length && depth > 0) {
    const ch = query[i];
    if (ch === "{") depth += 1;
    else if (ch === "}") depth -= 1;
    else if (depth === 1) own += ch;
    i += 1;
  }
  assert.equal(depth, 0, `unbalanced selection of ${name}`);
  return own.split(/\s+/).filter(Boolean);
}

/** `some_field` as async-graphql exposes it: `someField`. */
function graphqlFieldName(rustField) {
  return rustField.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
}

test("the storefront page query selects the published landing and never the authoring document", () => {
  assert.deepEqual(selectedFields(STOREFRONT_PAGE_QUERY, "landing"), [
    "locale",
    "html",
    "css",
    "artifactHash",
  ]);
  // `body` is the Page Builder authoring document: not sanitized for rendering,
  // and returned by the server only to callers that may save or publish it.
  const pageFields = selectedFields(STOREFRONT_PAGE_QUERY, "pageBySlug");
  assert.ok(pageFields.includes("landing"));
  assert.ok(!pageFields.includes("body"), "the storefront query must not request the body");
  assert.ok(!/\bbody\b|\bcontent\b/.test(STOREFRONT_PAGE_QUERY));
});

test("the landing selection is the server's published landing type", async () => {
  const types = await readFile(
    new URL("../../../../../crates/modules/rustok-pages/src/graphql/types.rs", import.meta.url),
    "utf8"
  );
  const declaration = /pub struct GqlPublishedLanding \{([^}]*)\}/.exec(types);
  assert.ok(declaration, "rustok-pages no longer declares GqlPublishedLanding");
  const serverFields = [...declaration[1].matchAll(/^\s*pub (\w+): String,$/gm)].map((field) =>
    graphqlFieldName(field[1])
  );

  // Every field is a non-null string on the server, and the storefront selects all of them.
  assert.deepEqual(selectedFields(STOREFRONT_PAGE_QUERY, "landing").sort(), serverFields.sort());
  assert.match(types, /async fn landing\(&self, ctx: &Context<'_>\) -> Result<Option<GqlPublishedLanding>>/);
});

test("readStorefrontPage returns the page with its landing, or null without a page", () => {
  const landing = {
    locale: "en",
    html: "<h1>Published</h1>",
    css: "h1 { color: red; }",
    artifactHash: "a".repeat(64),
  };
  const page = {
    id: PAGE_ID,
    version: 3,
    status: "published",
    effectiveLocale: "en",
    translation: null,
    landing,
  };

  assert.deepEqual(readStorefrontPage({ data: { selectedPage: page } }), page);
  assert.equal(readStorefrontPage({ data: { selectedPage: page } }).landing, landing);
  assert.equal(
    readStorefrontPage({ data: { selectedPage: { ...page, landing: null } } }).landing,
    null
  );
  assert.equal(readStorefrontPage({ data: { selectedPage: null } }), null);
  assert.equal(readStorefrontPage({}), null);
});

test("readStorefrontPage fails on a GraphQL error even when partial data came with it", () => {
  const partial = {
    id: PAGE_ID,
    version: 3,
    status: "published",
    effectiveLocale: "en",
    translation: null,
    landing: null,
  };

  // A landing lost to a resolver error must not read as "this page has no landing".
  assert.throws(
    () =>
      readStorefrontPage({
        data: { selectedPage: partial },
        errors: [{ message: "landing artifact integrity check failed" }],
      }),
    /landing artifact integrity check failed/
  );
  assert.throws(() => readStorefrontPage({ errors: [{ message: "" }] }), /Page query failed/);
});

test("the storefront route renders the landing of the page read and nothing else", async () => {
  const api = await readFile(new URL("./api/pages.ts", import.meta.url), "utf8");
  const route = await readFile(
    new URL("../../../src/app/[locale]/pages/[slug]/page.tsx", import.meta.url),
    "utf8"
  );

  assert.ok(api.includes("query: STOREFRONT_PAGE_QUERY"));
  assert.ok(api.includes("return readStorefrontPage(response);"));
  assert.ok(route.includes("const nonce = await getRequestCspNonce();"));
  assert.ok(
    route.includes(
      "<PageBuilderContent html={landing.html} css={landing.css} nonce={nonce} />"
    )
  );
  assert.ok(!/\bbody\b/.test(route), "the route must not read the page body");
  // The landing comes with the page: no second request, no artifact document to parse.
  for (const source of [api, route]) {
    assert.ok(!/\/api\/pages\/|artifact\b|\bfetch\(/i.test(source.replace(/artifactHash/g, "")));
  }
});

test("scopeLandingCss wraps and remaps root body/html selectors to :scope", () => {
  const raw = `body { margin: 0; background: #fff; }
html, body { height: 100%; }
.hero { color: red; }
tbody { display: table-row-group; }
.some-body { color: blue; }`;

  const scoped = scopeLandingCss(raw, ".fly-page-container");

  assert.ok(scoped.startsWith("@scope (.fly-page-container) {"));
  assert.ok(scoped.includes(":scope {\n    isolation: isolate;"));
  assert.ok(scoped.includes(":scope { margin: 0; background: #fff; }"));
  assert.ok(scoped.includes(":scope, :scope { height: 100%; }"));
  assert.ok(scoped.includes(".hero { color: red; }"));
  // Ensure tbody and .some-body are NOT accidentally remapped
  assert.ok(scoped.includes("tbody { display: table-row-group; }"));
  assert.ok(scoped.includes(".some-body { color: blue; }"));
});

test("scopeLandingCss handles empty inputs and already scoped CSS gracefully", () => {
  assert.equal(scopeLandingCss(""), "");
  assert.equal(scopeLandingCss("   "), "");

  // Its own output passes through unchanged, so scoping twice is harmless.
  const alreadyScoped = "@scope (.test) { :scope { color: green; } }";
  assert.equal(scopeLandingCss(alreadyScoped, ".test"), alreadyScoped);
  const once = scopeLandingCss("body { margin: 0; } .hero { color: red; }");
  assert.equal(scopeLandingCss(once), once);
});

test("scopeLandingCss never passes through CSS that only starts with @scope", () => {
  // A block scoped to something else is confined to the landing like any other rule.
  const foreign = "@scope (.test) { :scope { color: green; } }";
  const wrappedForeign = scopeLandingCss(foreign, ".other");
  assert.ok(wrappedForeign.startsWith("@scope (.other) {"));
  assert.ok(wrappedForeign.includes(foreign));

  // Rules after a leading @scope block used to reach the host page unscoped.
  const trailing = "@scope (.fly-page-container) { a { color: red; } } header { display: none; }";
  const wrappedTrailing = scopeLandingCss(trailing, ".fly-page-container");
  assert.notEqual(wrappedTrailing, trailing);
  assertSingleScopeBlock(wrappedTrailing);
  assert.ok(wrappedTrailing.includes("header { display: none; }"));
});

test("scopeLandingCss output is one complete @scope block for unbalanced input", () => {
  const cases = [
    // A stray `}` would close the wrapper and release the rest onto the host page.
    "} header { display: none; }",
    ".a { color: red; } } header { display: none; }",
    // Unclosed blocks, strings, comments and url() tokens.
    ".a { color: red;",
    "@media (min-width: 1px) { .a { color: red;",
    '.a { content: "unterminated',
    ".a { color: red; } /* unterminated",
    ".a { background: url(unterminated",
    ".a { width: calc((1px + 2px) ",
    ".a { color: red; } \\",
    // Braces that are not structure.
    '.a::before { content: "}"; } header { display: none; }',
    ".a::before { content: '\\'}'; } .b { color: blue; }",
    ".a { background: url(x}y.png); } .b { color: blue; }",
    ".a { background: url(x{y.png); } } header { display: none; }",
    ".a\\} { color: red; } .b { color: blue; }",
    "/* } */ .a { color: red; }",
    // A newline ends an unterminated string; the `}` after it is structure again.
    '.a { content: "open\n} } header { display: none; }',
    // A `}` inside an open parenthesis is an ordinary token to the parser.
    ".a { width: calc(1px } .b { color: blue; }",
  ];
  for (const css of cases) {
    assertSingleScopeBlock(scopeLandingCss(css), css);
  }
  assert.ok(!scopeLandingCss("} header { display: none; }").includes("}\n} header"));
});

test("scopeLandingCss closes strings by CSS escape rules, not by the previous character", () => {
  // `"\\"` is a complete string holding one backslash: the quote after it closes the string.
  const scoped = scopeLandingCss('.a::before { content: "\\\\"; } body { margin: 0; }');
  assert.ok(scoped.includes('.a::before { content: "\\\\"; }'));
  assert.ok(scoped.includes(":scope { margin: 0; }"));

  // `"\""` holds an escaped quote: the string continues past it.
  const escapedQuote = scopeLandingCss('.a::before { content: "\\" body {"; } body { margin: 0; }');
  assert.ok(escapedQuote.includes('content: "\\" body {";'));
  assert.ok(escapedQuote.includes(":scope { margin: 0; }"));
});

test("scopeLandingCss matches the vectors shared with the Leptos storefront", async () => {
  // The Rust twin (`scope_landing_css`) asserts the same file, so the two storefronts confine a
  // stylesheet identically. The cases cover the inputs that once showed the scanner one block
  // structure and the browser another, releasing rules onto the host page.
  const vectors = JSON.parse(
    await readFile(
      new URL(
        "../../../../../crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json",
        import.meta.url
      ),
      "utf8"
    )
  );
  assert.ok(vectors.cases.length > 0);
  for (const selector of [".fly-page-container", "[data-fly-page-body]"]) {
    for (const { name, css, inner } of vectors.cases) {
      assert.equal(
        scopeLandingCss(css, selector),
        `@scope (${selector}) {\n  :scope {\n    isolation: isolate;\n  }\n${inner}\n}`,
        name
      );
    }
  }
});

test("scopeLandingCss output cannot end the <style> element it is rendered into", () => {
  const scoped = scopeLandingCss(
    '.a::after { content: "</style><script>alert(1)</script>"; } /* </STYLE> */ .b { color: red; }'
  );
  assert.ok(!/<\/style/i.test(scoped));
  assert.ok(scoped.includes('content: "<\\/style><script>alert(1)</script>";'));
  assert.ok(scoped.includes("/* <\\/STYLE> */ .b { color: red; }"));
  // Its own output still passes through unchanged.
  assert.equal(scopeLandingCss(scoped), scoped);
});

test("scopeLandingCss remaps root selectors only in selector position", () => {
  const scoped = scopeLandingCss(
    [
      "@keyframes body { from { opacity: 0; } to { opacity: 1; } }",
      ".a { animation: body 1s; }",
      "a[rel=html] { color: red; }",
      'a[data-kind="body"] { color: red; }',
      ".b { background: url(body.png); }",
      "div:not(body) > html { color: blue; }",
      "@media (min-width: 1px) { .c { color: red; .d { color: blue; } } body { margin: 0; } }",
    ].join("\n")
  );
  assert.ok(scoped.includes("@keyframes body { from { opacity: 0; } to { opacity: 1; } }"));
  assert.ok(scoped.includes(".a { animation: body 1s; }"));
  assert.ok(scoped.includes("a[rel=html] { color: red; }"));
  assert.ok(scoped.includes('a[data-kind="body"] { color: red; }'));
  assert.ok(scoped.includes(".b { background: url(body.png); }"));
  assert.ok(scoped.includes("div:not(:scope) > :scope { color: blue; }"));
  assert.ok(scoped.includes(".d { color: blue; } } :scope { margin: 0; } }"));
});

test("scopeLandingCss preserves comments with braces and does not remap #body", () => {
  const rawWithComments = `/* { comment with braces } */
#body { font-size: 14px; }
/* body in comment */
body { padding: 8px; }`;

  const scoped = scopeLandingCss(rawWithComments, ".fly-page-container");

  assert.ok(scoped.includes("/* { comment with braces } */"));
  assert.ok(scoped.includes("#body { font-size: 14px; }"));
  assert.ok(scoped.includes("/* body in comment */"));
  assert.ok(scoped.includes(":scope { padding: 8px; }"));
});

test("scopeLandingCss handles Cyrillic comments, nested media queries, and :root normalization", () => {
  const raw = `
/* Привет, мир! Комментарий на кириллице */
:root {
  --primary-color: #007bff;
}
body {
  font-family: 'Roboto', sans-serif;
  --custom-ident: body;
}
@media (max-width: 768px) {
  body {
    padding: 10px;
  }
}
@supports (display: flex) {
  @media (min-width: 1200px) {
    html, body {
      display: flex;
    }
  }
}
`;

  const scoped = scopeLandingCss(raw, ".fly-page-container");

  assert.ok(scoped.includes("/* Привет, мир! Комментарий на кириллице */"));
  assert.ok(scoped.includes(":scope {\n  --primary-color: #007bff;\n}"));
  assert.ok(scoped.includes(":scope {\n  font-family: 'Roboto', sans-serif;\n  --custom-ident: body;\n}"));
  assert.ok(scoped.includes("@media (max-width: 768px) {\n  :scope {\n    padding: 10px;\n  }\n}"));
  assert.ok(scoped.includes("@media (min-width: 1200px) {\n    :scope, :scope {\n      display: flex;\n    }\n  }"));
});

test("fly-actions runtime is the canonical asset and handles every non-native action kind", async () => {
  const vendored = await readFile(new URL("./components/fly-actions.js", import.meta.url));
  const canonical = await readFile(
    new URL("../../../../../crates/ui/fly/browser/assets/fly-actions.js", import.meta.url)
  );

  // The Next package consumes the canonical runtime byte-for-byte instead of a
  // parallel TypeScript implementation. The Rust-side contract lives in
  // crates/ui/fly/browser/tests/action_runtime_contract.rs; the npm-level
  // structural checks live in scripts/verify-fly-actions-parity.mjs.
  assert.ok(
    vendored.equals(canonical),
    "components/fly-actions.js must stay a byte-exact copy of crates/ui/fly/browser/assets/fly-actions.js"
  );

  // Every non-native ComponentAction kind (navigate_page, navigate_url and
  // submit_form are materialized as href/form attributes and never reach the
  // runtime) must have a case in the runtime's handleAction switch.
  const kinds = new Set();
  for (const match of canonical.toString("utf8").matchAll(/case\s+"([a-z_]+)"\s*:/g)) {
    kinds.add(match[1]);
  }
  assert.deepEqual([...kinds].sort(), [
    "animate",
    "emit_event",
    "provider_action",
    "scroll_to",
    "switch_slide",
    "switch_tab",
    "toggle_accordion",
    "toggle_visibility",
  ]);
});




test("buyer handoff preserves explicit selectors and never guesses a product SKU or price", () => {
  const sku = "11111111-1111-4111-8111-111111111111";
  const origin = "https://store.example";
  assert.equal(
    buyerHandoffHref({ buyerHref: "/ru/products/cup?currency=AED#buy", variantId: sku.toUpperCase(), quantity: "2" }, origin),
    `/ru/products/cup?currency=AED&variant_id=${sku}&quantity=2#buy`,
  );
  assert.equal(buyerHandoffHref({ buyerHref: "/ru/products/cup", variantId: sku }, origin),
    `/ru/products/cup?variant_id=${sku}&quantity=1`);
  assert.equal(buyerHandoffHref({ buyerHref: "/zh-Hant/products/cup", variantId: sku }, origin),
    `/zh-Hant/products/cup?variant_id=${sku}&quantity=1`);
  assert.equal(buyerHandoffHref({ buyerHref: "/fil/products/cup", variantId: sku }, origin),
    `/fil/products/cup?variant_id=${sku}&quantity=1`);
  for (const bad of [
    { buyerHref: "/ru/products/cup", productId: sku },
    { buyerHref: "/ru/products/cup", id: sku },
    { buyerHref: "https://evil.example", variantId: sku },
    { buyerHref: "//evil.example", variantId: sku },
    { buyerHref: "/ru/pages/not-a-product", variantId: sku },
    { buyerHref: "/ru/modules/products?handle=cup", variantId: sku },
    { buyerHref: "/ru/products", variantId: sku },
    { buyerHref: "/ru/products/cup/other", variantId: sku },
    { buyerHref: "/ru/PRODUCTS/cup", variantId: sku },
    { buyerHref: "/ru/products/%ZZ", variantId: sku },
    { buyerHref: "/ru/products/cup%2Fother", variantId: sku },
    { buyerHref: "/en%2Ffr/products/cup", variantId: sku },
    { buyerHref: "/ru/products/cup?handle=other", variantId: sku },
    { buyerHref: "/ru/products/cup?variant_id=other", variantId: sku },
    { buyerHref: "/ru/products/cup?cart_id=other", variantId: sku },
    { buyerHref: "/ru/products/cup?axis.x=y", variantId: sku },
    { buyerHref: "/ru/products/cup?currency=AED&currency=USD", variantId: sku },
    ...[0, -1, 0.5, NaN, Infinity, 2147483648, null, true, "-1", "1e2", " 1"].map(quantity => ({
      buyerHref: "/ru/products/cup", variantId: sku, quantity,
    })),
  ]) assert.equal(buyerHandoffHref(bad, origin), null, JSON.stringify(bad));
  assert.equal(buyerHandoffHref({ buyerHref: "/ru/products/cup", variantId: sku, quantity: 2147483647 }, origin),
    `/ru/products/cup?variant_id=${sku}&quantity=2147483647`);
  assert.equal(safeNavigationHref("javascript:alert(1)"), null);
  assert.equal(safeNavigationHref("/\\evil.example"), null);
});
