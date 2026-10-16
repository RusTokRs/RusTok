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
import { parseArtifactHtml } from "./api/artifact.ts";

test("parseArtifactHtml separates style and body tags correctly", () => {
  const sampleDocument = `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>Landing</title>
  <style>
    .hero { background: #000; color: #fff; }
    .btn:hover { opacity: 0.9; }
  </style>
</head>
<body>
  <div class="hero">
    <h1>Welcome to RusTok</h1>
    <button data-fly-action='{"kind":"emit_event","event":"add_to_cart","payload":{"productId":"prod_1"}}'>Buy</button>
  </div>
</body>
</html>`;

  const parsed = parseArtifactHtml(sampleDocument);

  assert.ok(parsed.css.includes(".hero { background: #000; color: #fff; }"));
  assert.ok(parsed.css.includes(".btn:hover"));
  assert.ok(!parsed.css.includes("<style>"));
  assert.ok(!parsed.css.includes("</style>"));

  assert.ok(parsed.html.includes("<h1>Welcome to RusTok</h1>"));
  assert.ok(parsed.html.includes("data-fly-action"));
  assert.ok(!parsed.html.includes("<body"));
  assert.ok(!parsed.html.includes("</body>"));
  assert.ok(!parsed.html.includes("<!doctype html>"));
});

test("parseArtifactHtml handles documents without separate style tag gracefully", () => {
  const plainMarkup = `<div class="content"><p>Simple snippet</p></div>`;
  const parsed = parseArtifactHtml(plainMarkup);

  assert.equal(parsed.css, "");
  assert.equal(parsed.html, plainMarkup);
});
