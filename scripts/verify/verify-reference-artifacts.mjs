#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

function fail(message) {
  console.error(`[reference] ${message}`);
  process.exit(1);
}

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
  } catch (error) {
    fail(`${file}: invalid JSON: ${error.message}`);
  }
}

function latestReferenceDir(inputPath) {
  const absolute = path.resolve(inputPath);
  if (fs.existsSync(path.join(absolute, "manifest.json"))) {
    return absolute;
  }
  if (!fs.existsSync(absolute)) {
    fail(`${absolute}: path does not exist`);
  }
  const entries = fs
    .readdirSync(absolute, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(absolute, entry.name))
    .filter((dir) => fs.existsSync(path.join(dir, "manifest.json")))
    .sort();
  if (entries.length === 0) {
    fail(`${absolute}: no reference artifact directories with manifest.json found`);
  }
  return entries[entries.length - 1];
}

function requireFile(root, relativePath) {
  const file = path.join(root, relativePath);
  if (!fs.existsSync(file)) {
    fail(`${relativePath}: missing`);
  }
  const stat = fs.statSync(file);
  if (!stat.isFile() || stat.size === 0) {
    fail(`${relativePath}: empty or not a file`);
  }
  return file;
}

const root = latestReferenceDir(process.argv[2] ?? "artifacts/reference");

const manifestFile = requireFile(root, "manifest.json");
const openapiJsonFile = requireFile(root, "openapi/openapi.json");
requireFile(root, "openapi/openapi.yaml");
const introspectionFile = requireFile(root, "graphql/introspection.json");
const sdlFile = requireFile(root, "graphql/schema.graphql");

const manifest = readJson(manifestFile);
if (manifest.schema !== "rustok.reference_artifacts.v1") {
  fail("manifest.json: unexpected or missing schema");
}
for (const expected of [
  "openapi/openapi.json",
  "openapi/openapi.yaml",
  "graphql/introspection.json",
  "graphql/schema.graphql",
]) {
  if (!manifest.files?.includes(expected)) {
    fail(`manifest.json: files does not include ${expected}`);
  }
}

const openapi = readJson(openapiJsonFile);
if (!openapi.openapi || !String(openapi.openapi).startsWith("3.")) {
  fail("openapi/openapi.json: missing OpenAPI 3.x version");
}
if (!openapi.paths || Object.keys(openapi.paths).length === 0) {
  fail("openapi/openapi.json: missing paths");
}
if (!openapi.components || Object.keys(openapi.components.schemas ?? {}).length === 0) {
  fail("openapi/openapi.json: missing component schemas");
}

const introspection = readJson(introspectionFile);
if (Array.isArray(introspection.errors) && introspection.errors.length > 0) {
  fail(`graphql/introspection.json: GraphQL errors returned: ${JSON.stringify(introspection.errors)}`);
}
const schema = introspection.data?.__schema;
if (!schema) {
  fail("graphql/introspection.json: missing data.__schema");
}
if (!schema.queryType?.name) {
  fail("graphql/introspection.json: missing queryType");
}
if (!Array.isArray(schema.types) || schema.types.length === 0) {
  fail("graphql/introspection.json: missing types");
}
const objectWithFields = schema.types.find(
  (type) => type.kind === "OBJECT" && Array.isArray(type.fields) && type.fields.length > 0,
);
if (!objectWithFields) {
  fail("graphql/introspection.json: no object type contains fields");
}
const fieldWithArgs = schema.types
  .flatMap((type) => Array.isArray(type.fields) ? type.fields : [])
  .find((field) => Array.isArray(field.args));
if (!fieldWithArgs) {
  fail("graphql/introspection.json: field argument metadata is missing");
}

const sdl = fs.readFileSync(sdlFile, "utf8");
if (!/\btype\s+Query\b/.test(sdl)) {
  fail("graphql/schema.graphql: missing type Query");
}

console.log(`[reference] verified ${root}`);
