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

import {
  buildDeterministicSeoRoute,
  serializeStructuredData,
  normalizeRobots,
  buildVerification,
} from "./utils.ts";

import {
  getSiteUrl,
  localizedPath,
  resolveLocale,
  defaultLocale,
  locales,
} from "./site.ts";

test("site url trims trailing slashes and resolves fallback", () => {
  const url = getSiteUrl();
  assert.ok(url.startsWith("http"));
  assert.ok(!url.endsWith("/"));
});

test("localizedPath normalizes routes, strips existing locale prefixes and handles external URLs", () => {
  // External URLs are untouched
  assert.equal(localizedPath("en", "https://example.com/item"), "https://example.com/item");
  assert.equal(localizedPath("ru", "http://example.com/test"), "http://example.com/test");

  // Root path
  assert.equal(localizedPath("en", "/"), "/en");
  assert.equal(localizedPath("ru", "/"), "/ru");

  // Normal relative path without leading slash
  assert.equal(localizedPath("en", "catalog"), "/en/catalog");

  // Path with existing locale prefix (prevent /en/en/products)
  assert.equal(localizedPath("en", "/en/products"), "/en/products");
  assert.equal(localizedPath("ru", "/en/products"), "/ru/products");
  assert.equal(localizedPath("en", "/ru/blog/sample"), "/en/blog/sample");
});

test("resolveLocale normalizes locale candidates and falls back gracefully", () => {
  assert.equal(defaultLocale, "en");
  assert.deepEqual([...locales], ["en", "ru"]);

  assert.equal(resolveLocale(undefined), "en");
  assert.equal(resolveLocale(null), "en");
  assert.equal(resolveLocale(""), "en");
  assert.equal(resolveLocale("en"), "en");
  assert.equal(resolveLocale("ru"), "ru");
  assert.equal(resolveLocale("RU"), "ru");
  assert.equal(resolveLocale("ru_RU"), "ru");
  assert.equal(resolveLocale("en-US"), "en");
  assert.equal(resolveLocale("de"), "en"); // unsupported falls back to default
});

test("buildDeterministicSeoRoute maps standard module routes deterministically", () => {
  // Products route
  assert.equal(
    buildDeterministicSeoRoute({ route: "/products/ergonomic-chair" }),
    "/modules/product?handle=ergonomic-chair&slug=ergonomic-chair",
  );
  assert.equal(
    buildDeterministicSeoRoute({ route: "/ru/products/chair-1" }),
    "/modules/product?handle=chair-1&slug=chair-1",
  );

  // Blog route
  assert.equal(
    buildDeterministicSeoRoute({ route: "/blog/first-post" }),
    "/modules/blog?slug=first-post",
  );
  assert.equal(
    buildDeterministicSeoRoute({ route: "/en/blog/news-2026" }),
    "/modules/blog?slug=news-2026",
  );

  // Pages route
  assert.equal(
    buildDeterministicSeoRoute({ route: "/pages/terms-of-service" }),
    "/modules/pages?slug=terms-of-service",
  );
  assert.equal(
    buildDeterministicSeoRoute({ route: "/ru/pages/about-us" }),
    "/modules/pages?slug=about-us",
  );

  // RouteSegment mode with query sorting and lang exclusion
  assert.equal(
    buildDeterministicSeoRoute({
      routeSegment: "catalog",
      query: { category: "shoes", lang: "ru", brand: "nike" },
    }),
    "/modules/catalog?brand=nike&category=shoes",
  );

  // Array query value takes first valid
  assert.equal(
    buildDeterministicSeoRoute({
      routeSegment: "search",
      query: { q: ["laptop", "tablet"] },
    }),
    "/modules/search?q=laptop",
  );

  // Unmapped route preserves leading slash and query
  assert.equal(
    buildDeterministicSeoRoute({ route: "custom-path", query: { ref: "promo" } }),
    "/custom-path?ref=promo",
  );
});

test("serializeStructuredData produces safe escaped JSON-LD script payloads", () => {
  // Non-objects return null
  assert.equal(serializeStructuredData(null), null);
  assert.equal(serializeStructuredData(undefined), null);
  assert.equal(serializeStructuredData("string"), null);
  assert.equal(serializeStructuredData(123), null);

  // Valid object
  const valid = { "@type": "Product", name: "Chair" };
  assert.equal(serializeStructuredData(valid), '{"@type":"Product","name":"Chair"}');

  // XSS and HTML characters are escaped
  const xss = {
    danger: "<script>alert('xss')</script>",
    entity: "Fish & Chips",
    lineSeparators: "Line 1\u2028Line 2\u2029",
  };
  const serialized = serializeStructuredData(xss);
  assert.ok(serialized);
  assert.ok(!serialized.includes("<script>"));
  assert.ok(serialized.includes("\\u003cscript\\u003e"));
  assert.ok(!serialized.includes("Fish & Chips"));
  assert.ok(serialized.includes("Fish \\u0026 Chips"));
});

test("normalizeRobots maps robots schema to Next.js metadata format", () => {
  // Default when undefined
  assert.deepEqual(normalizeRobots(), { index: true, follow: true });

  // Customized flags
  const robots = normalizeRobots({
    index: false,
    follow: true,
    noarchive: true,
    nosnippet: true,
    noimageindex: true,
    notranslate: true,
    maxSnippet: 50,
    maxImagePreview: "large",
    maxVideoPreview: 30,
  });

  assert.equal(robots.index, false);
  assert.equal(robots.follow, true);
  assert.equal(robots.noarchive, true);
  assert.equal(robots.nosnippet, true);
  assert.equal(robots.noimageindex, true);
  assert.equal(robots.notranslate, true);
  assert.equal(robots["max-snippet"], 50);
  assert.equal(robots["max-image-preview"], "large");
  assert.equal(robots["max-video-preview"], 30);

  // Invalid maxImagePreview is dropped
  const invalidPreview = normalizeRobots({
    index: true,
    follow: true,
    maxImagePreview: "invalid_value",
  });
  assert.equal(invalidPreview["max-image-preview"], undefined);
});

test("buildVerification filters empty tags and formats search engine verification codes", () => {
  assert.equal(buildVerification(), undefined);

  const verification = buildVerification({
    google: ["google-code-1"],
    yandex: ["yandex-code-1"],
    yahoo: ["yahoo-code-1"],
    other: [
      { name: "custom-bot", value: "token-123" },
      { name: "   ", value: "invalid" },
      { name: "invalid-empty-value", value: "   " },
    ],
  });

  assert.ok(verification);
  assert.deepEqual(verification.google, ["google-code-1"]);
  assert.deepEqual(verification.yandex, ["yandex-code-1"]);
  assert.deepEqual(verification.yahoo, ["yahoo-code-1"]);
  assert.deepEqual(verification.other, { "custom-bot": "token-123" });
});

