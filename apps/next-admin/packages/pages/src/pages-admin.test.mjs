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
import { builderUrlForPage, storefrontUrlForPage } from "./api/urls.ts";

test("builderUrlForPage constructs deep link to Leptos admin editor", () => {
  const url = builderUrlForPage("http://localhost:5150", "page-123");
  assert.equal(url, "http://localhost:5150/modules/pages?pageId=page-123");

  const urlWithTrailingSlash = builderUrlForPage("http://localhost:5150/", "page-456");
  assert.equal(urlWithTrailingSlash, "http://localhost:5150/modules/pages?pageId=page-456");
});

test("storefrontUrlForPage constructs canonical storefront link for published pages", () => {
  const url = storefrontUrlForPage("http://localhost:3000", "about", "en", "published");
  assert.equal(url, "http://localhost:3000/en/pages/about");

  const urlRu = storefrontUrlForPage("http://localhost:3000/", "contacts", "ru", "published");
  assert.equal(urlRu, "http://localhost:3000/ru/pages/contacts");
});

test("storefrontUrlForPage constructs draft preview link with secret for draft pages", () => {
  const url = storefrontUrlForPage(
    "http://localhost:3000",
    "spring-sale",
    "ru",
    "draft",
    "secret123"
  );
  assert.equal(
    url,
    "http://localhost:3000/api/draft?slug=spring-sale&locale=ru&secret=secret123"
  );
});

test("storefrontUrlForPage falls back to canonical URL when draft has no secret", () => {
  const url = storefrontUrlForPage("http://localhost:3000", "promo", "en", "draft");
  assert.equal(url, "http://localhost:3000/en/pages/promo");
});
