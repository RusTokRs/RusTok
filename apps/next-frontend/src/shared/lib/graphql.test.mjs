/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { storefrontGraphql } from "./graphql.ts";

const query =
  "query ForumTest { forumStorefrontAudienceTopics { nextCursor } }";

test("Apollo 4 GraphQL errors survive errorPolicy all, even with partial data", async () => {
  const original = globalThis.fetch;
  const headers = [];
  const caches = [];
  globalThis.fetch = async (_request, init) => {
    headers.push(new Headers(init.headers));
    caches.push(init.cache);
    return new Response(
      JSON.stringify({
        data: { forumStorefrontAudienceTopics: null },
        errors: [
          {
            message: "Audience denied",
            path: ["forumStorefrontAudienceTopics"],
          },
        ],
      }),
      { status: 200, headers: { "Content-Type": "application/json" } },
    );
  };
  try {
    const response = await storefrontGraphql({
      query,
      baseUrl: "http://test-forum-graphql.invalid",
      tenant: "demo",
      token: "user-token",
    });
    assert.deepEqual(response.errors, [{ message: "Audience denied" }]);
    assert.equal(response.data?.forumStorefrontAudienceTopics, null);
    assert.equal(headers[0].get("Authorization"), "Bearer user-token");
    assert.equal(headers[0].get("X-Tenant-Slug"), "demo");
    assert.equal(caches[0], "no-store");
  } finally {
    globalThis.fetch = original;
  }
});

test("an anonymous request never borrows the token from a previous request", async () => {
  const original = globalThis.fetch;
  const headers = [];
  globalThis.fetch = async (_request, init) => {
    headers.push(new Headers(init.headers));
    return new Response(
      JSON.stringify({
        data: { forumStorefrontAudienceTopics: { nextCursor: null } },
      }),
      {
        status: 200,
        headers: { "Content-Type": "application/json" },
      },
    );
  };
  try {
    const authorized = await storefrontGraphql({
      query,
      baseUrl: "http://test-forum-isolation.invalid",
      token: "one-user",
    });
    const guest = await storefrontGraphql({
      query,
      baseUrl: "http://test-forum-isolation.invalid",
    });
    assert.equal(authorized.errors, undefined);
    assert.equal(guest.errors, undefined);
    assert.equal(headers[0].get("Authorization"), "Bearer one-user");
    assert.equal(headers[1].get("Authorization"), null);
  } finally {
    globalThis.fetch = original;
  }
});

const scopeProbeQuery =
  "query PricingScopeProbe($sku: String!) { scopeProbe(sku: $sku) }";

test("concurrent identical queries keep tenant and auth headers isolated", async () => {
  const originalFetch = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (_url, options) => {
    const headers = new Headers(options?.headers);
    const scope = `${headers.get("X-Tenant-Slug")}:${headers.get("Authorization")}`;
    calls.push(scope);
    // Keep the operation in flight while the other same-document reads start.
    await Promise.resolve();
    return new Response(JSON.stringify({ data: { scopeProbe: scope } }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  };
  try {
    const scopes = [
      { tenant: "tenant-a", token: "actor-a" },
      { tenant: "tenant-b", token: "actor-a" },
      { tenant: "tenant-a", token: "actor-b" },
    ];
    const results = await Promise.all(
      scopes.map((scope) =>
        storefrontGraphql({
          query: scopeProbeQuery,
          variables: { sku: "stable-sku" },
          ...scope,
          baseUrl: "https://scope-probe.invalid",
        }),
      ),
    );
    assert.equal(calls.length, scopes.length);
    for (const [index, scope] of scopes.entries()) {
      assert.equal(
        results[index].data.scopeProbe,
        `${scope.tenant}:Bearer ${scope.token}`,
      );
      assert.equal(results[index].errors, undefined);
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});
