import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:net";
import { fileURLToPath } from "node:url";
import { after, before, test } from "node:test";
import { join } from "node:path";

const appRoot = fileURLToPath(new URL("../../../", import.meta.url));
let child;
let origin;
let childOutput = "";

async function reservePort() {
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert.ok(address && typeof address === "object");
  await new Promise((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  return address.port;
}

async function waitForServer(url) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`next start exited early (${child.exitCode}): ${childOutput}`);
    }
    try {
      return await fetch(url, { headers: { Accept: "text/html" } });
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw new Error(`next start did not become ready: ${childOutput}`);
}

before(async () => {
  const port = await reservePort();
  origin = `http://127.0.0.1:${port}`;
  child = spawn(
    process.execPath,
    [join(appRoot, "node_modules/next/dist/bin/next"), "start", "--hostname", "127.0.0.1", "--port", String(port)],
    { cwd: appRoot, env: process.env, stdio: ["ignore", "pipe", "pipe"] },
  );
  child.stdout.setEncoding("utf8").on("data", (chunk) => (childOutput += chunk));
  child.stderr.setEncoding("utf8").on("data", (chunk) => (childOutput += chunk));
  const initialResponse = await waitForServer(origin);
  await initialResponse.arrayBuffer();
});

after(async () => {
  if (!child || child.exitCode !== null) return;
  child.kill("SIGTERM");
  const exited = await Promise.race([
    once(child, "exit").then(() => true),
    new Promise((resolve) => setTimeout(() => resolve(false), 3_000)),
  ]);
  if (!exited) child.kill("SIGKILL");
});

async function fetchHtml(path, headers = {}) {
  return fetch(new URL(path, origin), {
    headers: { Accept: "text/html", ...headers },
  });
}

function assertNonceResponse(response, { status = 200 } = {}) {
  assert.equal(response.status, status);
  assert.match(response.headers.get("cache-control") ?? "", /private/);
  assert.match(response.headers.get("cache-control") ?? "", /no-store/);

  const policy = response.headers.get("content-security-policy") ?? "";
  const policyNonces = new Set(
    [...policy.matchAll(/'nonce-([A-Za-z0-9+/=_-]+)'/g)].map((match) => match[1]),
  );
  assert.equal(policyNonces.size, 1, "the response policy must contain one nonce");

  return response.text().then((html) => {
    const htmlNonces = new Set(
      [...html.matchAll(/\bnonce="([A-Za-z0-9+/=_-]+)"/g)].map((match) => match[1]),
    );
    assert.ok(htmlNonces.size > 0, "SSR framework output must carry a nonce");
    assert.deepEqual(htmlNonces, policyNonces, "SSR and response policy nonces must match");
    return [...policyNonces][0];
  });
}

test("production HTML forwards the request nonce to CSP and SSR without caching", async () => {
  const first = await fetchHtml("/");
  const firstNonce = await assertNonceResponse(first);
  const second = await fetchHtml("/");
  const secondNonce = await assertNonceResponse(second);

  assert.notEqual(firstNonce, secondNonce, "each response must receive a fresh nonce");
});

test("a client-supplied nonce cannot control the enforced policy", async () => {
  const attackerNonce = "attacker-controlled-nonce";
  const response = await fetchHtml("/", { "x-nonce": attackerNonce });
  const issuedNonce = await assertNonceResponse(response);

  assert.notEqual(issuedNonce, attackerNonce);
});

test("HTML 404 responses, including dotted paths, receive the strict policy", async () => {
  const plainPath = await fetchHtml("/__csp-runtime-test-not-found__");
  await assertNonceResponse(plainPath, { status: 404 });

  const dottedPath = await fetchHtml("/__csp-runtime-test-not-found__.html");
  await assertNonceResponse(dottedPath, { status: 404 });
});

test("Next static assets remain cacheable and outside the document CSP", async () => {
  const document = await fetchHtml("/");
  const html = await document.text();
  const assetPath = html.match(/src="(\/_next\/static\/chunks\/[^\"]+\.js)"/)?.[1];
  assert.ok(assetPath, "the rendered document must reference a Next static script");

  const response = await fetch(new URL(assetPath, origin), {
    headers: { Accept: "*/*", "sec-fetch-dest": "script" },
  });
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-security-policy"), null);
  assert.match(response.headers.get("cache-control") ?? "", /immutable/);
});
