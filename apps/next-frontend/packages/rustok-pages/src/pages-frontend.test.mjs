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
import { parseArtifactHtml, pageCacheTags } from "./api/artifact.ts";
import { scopeLandingCss } from "./components/css-scope.ts";

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

test("parseArtifactHtml handles multiple style blocks across document", () => {
  const docWithMultipleStyles = `<!doctype html>
<html>
<head>
  <style>.first { color: blue; }</style>
  <style>.second { color: green; }</style>
</head>
<body>
  <div class="first second">Content</div>
</body>
</html>`;
  const parsed = parseArtifactHtml(docWithMultipleStyles);

  assert.ok(parsed.css.includes(".first { color: blue; }"));
  assert.ok(parsed.css.includes(".second { color: green; }"));
  assert.ok(parsed.html.includes('<div class="first second">Content</div>'));
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

  const alreadyScoped = "@scope (.test) { :scope { color: green; } }";
  assert.equal(scopeLandingCss(alreadyScoped, ".other"), alreadyScoped);
});

test("pageCacheTags generates canonical cache tags for ISR", () => {
  assert.deepEqual(pageCacheTags("about-us"), ["page-about-us", "pages"]);
  assert.deepEqual(pageCacheTags(""), ["pages"]);
  assert.deepEqual(pageCacheTags(), ["pages"]);
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



