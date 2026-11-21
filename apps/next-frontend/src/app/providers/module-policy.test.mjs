import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";
import React from "react";

async function load(path, mocks) {
  const source = await readFile(new URL(path, import.meta.url), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  const mockModule = { exports: {} };
  const require = (id) => {
    if (id === "react/jsx-runtime")
      return {
        jsx: React.createElement,
        jsxs: React.createElement,
        Fragment: React.Fragment,
      };
    if (id in mocks) return mocks[id];
    throw new Error(`Unexpected import ${id}`);
  };
  new Function("module", "exports", "require", "process", js)(
    mockModule,
    mockModule.exports,
    require,
    { env: {} },
  );
  return { api: mockModule.exports, source };
}

test("storefront availability rejects missing scope, partial errors, invalid responses and capped lists", async () => {
  let result;
  const calls = [];
  const { api, source } = await load("../../shared/api/modules.ts", {
    "@/shared/lib/graphql": {
      storefrontGraphql: async (args) => {
        calls.push(args);
        return result;
      },
    },
  });
  const server = await readFile(
    new URL("../../../../server/src/graphql/queries.rs", import.meta.url),
    "utf8",
  );
  assert.match(server, /limit\.unwrap_or\(100\)\.clamp\(1, 100\)/);
  assert.match(source, /enabledModules\(limit: 100\)/);
  await assert.rejects(api.fetchEnabledModules(null), /scope/);
  await assert.rejects(api.fetchEnabledModules("  "), /scope/);
  assert.equal(calls.length, 0);
  for (const response of [
    { errors: [{ message: "denied" }], data: { enabledModules: ["forum"] } },
    {},
    { data: { enabledModules: null } },
    { data: { enabledModules: ["ok", 3] } },
    { data: { enabledModules: [""] } },
  ]) {
    result = response;
    await assert.rejects(api.fetchEnabledModules("demo"));
  }
  const modules = Array.from({ length: 101 }, (_, i) => `module_${i}`);
  result = { data: { enabledModules: modules.slice(0, 99) } };
  assert.equal((await api.fetchEnabledModules("demo")).length, 99);
  for (const count of [100, 101]) {
    result = { data: { enabledModules: modules.slice(0, count) } };
    await assert.rejects(api.fetchEnabledModules("demo"), /incomplete/);
  }
  result = { data: { enabledModules: ["forum", "blog", "forum"] } };
  assert.deepEqual(await api.fetchEnabledModules("demo"), ["blog", "forum"]);
  result = { data: { enabledModules: [] } };
  assert.deepEqual(await api.fetchEnabledModules("demo"), []);
  assert.equal(calls.at(-1).tenant, "demo");
});

test("provider and guard show policy errors instead of an empty policy or not-found", async () => {
  let modules = ["forum"];
  const fetchEnabledModules = async () => {
    if (modules instanceof Error) throw modules;
    return modules;
  };
  const errorView = () => null;
  const clientProvider = () => null;
  let notFoundCount = 0;
  const mocks = {
    "@/shared/api/modules": {
      fetchEnabledModules,
      getStorefrontTenantSlug: () => "demo",
    },
    "@/shared/lib/enabled-modules-context": {
      EnabledModulesClientProvider: clientProvider,
    },
    "./module-policy-error": { ModulePolicyError: errorView },
    "next/navigation": {
      notFound: () => {
        notFoundCount++;
        throw new Error("not found");
      },
    },
  };
  const { api: provider } = await load("./enabled-modules-provider.tsx", mocks);
  const { api: guard } = await load("./module-guard.tsx", mocks);
  const children = React.createElement("span", null, "content");
  const props = { slug: "forum", children };
  assert.equal(
    (await provider.EnabledModulesProvider({ children })).type,
    clientProvider,
  );
  assert.equal((await guard.ModuleGuard(props)).props.children, children);
  modules = new Error("backend down");
  assert.equal(
    (await provider.EnabledModulesProvider({ children })).type,
    errorView,
  );
  assert.equal(
    (await guard.ModuleGuard({ ...props, fallback: "disabled" })).type,
    errorView,
  );
  assert.equal(notFoundCount, 0);
  modules = [];
  assert.equal(
    (await guard.ModuleGuard({ ...props, fallback: "disabled" })).props
      .children,
    "disabled",
  );
  await assert.rejects(guard.ModuleGuard(props), /not found/);
  assert.equal(notFoundCount, 1);
});

test("localized availability error retries the current route", async () => {
  let refreshed = 0;
  const { api } = await load("./module-policy-error.tsx", {
    "@rustok/next-fluent/client": { useTranslations: () => (key) => key },
    "next/navigation": {
      useRouter: () => ({
        refresh: () => {
          refreshed++;
        },
      }),
    },
  });
  const view = api.ModulePolicyError();
  const children = view.props.children.props.children;
  assert.equal(children[0].props.children, "title");
  assert.equal(children[1].props.children, "description");
  assert.equal(children[2].props.children, "retry");
  children[2].props.onClick();
  assert.equal(refreshed, 1);
});

test("home composition shows a retryable policy error when its own read fails", async () => {
  const errorView = () => null;
  const translations = Object.assign(() => "", { raw: () => [] });
  const { api } = await load("../[locale]/page.tsx", {
    "lucide-react": {},
    "@rustok/next-fluent/server": { getTranslations: async () => translations },
    "@/modules": {
      getModulesForSlot: () => {
        assert.fail("no composition without policy");
      },
    },
    "@/shared/api/modules": {
      getStorefrontTenantSlug: () => "demo",
      fetchEnabledModules: async () => {
        throw new Error("unavailable");
      },
    },
    "@/shared/seo/metadata": {},
    "@/shared/seo/runtime": {},
    "@/shared/security/nonce": {},
    "@/shared/lib/graphql": {},
    "@/shared/ui/base/button": {},
    "@/app/providers/module-policy-error": { ModulePolicyError: errorView },
  });
  const view = await api.default({
    params: Promise.resolve({ locale: "en" }),
    searchParams: Promise.resolve({}),
  });
  assert.equal(view.type, errorView);
});
