#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json";
const contract = JSON.parse(readFileSync(path.join(repoRoot, contractPath), "utf8"));

function fail(message) {
  throw new Error(`Pages inline edit build snapshot capture failed: ${message}`);
}

function parseArguments(argv) {
  const options = {
    adminDist: path.join(repoRoot, "apps/admin/dist"),
    bootstrap: path.join(repoRoot, "target/site/assets/pages-inline-edit-bootstrap.js"),
    clientJs: path.join(
      repoRoot,
      "target/site/assets/pages-inline-edit/rustok_storefront.js",
    ),
    clientWasm: path.join(
      repoRoot,
      "target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm",
    ),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: capture-pages-inline-edit-build-snapshot.mjs " +
          "--profile build-a|build-b --server-binary FILE --trunk FILE " +
          "--wasm-bindgen FILE --command-log FILE --output FILE " +
          "[--source-commit SHA] [--admin-dist DIR] [--bootstrap FILE] " +
          "[--client-js FILE] [--client-wasm FILE]",
      );
      process.exit(0);
    }
    if (
      [
        "--profile",
        "--server-binary",
        "--trunk",
        "--wasm-bindgen",
        "--command-log",
        "--output",
        "--source-commit",
        "--admin-dist",
        "--bootstrap",
        "--client-js",
        "--client-wasm",
      ].includes(argument)
    ) {
      const value = argv[index + 1];
      if (!value) fail(`${argument} requires a value`);
      const key = argument
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      options[key] = value;
      index += 1;
      continue;
    }
    fail(`unknown argument ${argument}`);
  }
  return options;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/u.test(value)) {
    fail(`${label} must be a lowercase 40-character git commit`);
  }
  return value;
}

function currentCommit() {
  return requireCommit(
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
    "git HEAD",
  );
}

function normalizePath(value) {
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function fileRecord(file, label, { executable = false } = {}) {
  const absolute = normalizePath(file);
  if (!existsSync(absolute)) fail(`${label} is missing: ${absolute}`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file: ${absolute}`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0) fail(`${label} must be non-empty: ${absolute}`);
  if (executable && (stats.mode & 0o111) === 0) {
    fail(`${label} must be executable: ${absolute}`);
  }
  const bytes = readFileSync(absolute);
  const relative = path.relative(repoRoot, absolute);
  return {
    path: relative.startsWith("..") ? absolute : relative,
    bytes: stats.size,
    sha256: sha256(bytes),
    executable: (stats.mode & 0o111) !== 0,
  };
}

function directoryManifest(directory) {
  const root = normalizePath(directory);
  if (!existsSync(root)) fail(`admin dist directory is missing: ${root}`);
  const rootStats = lstatSync(root);
  if (rootStats.isSymbolicLink() || !rootStats.isDirectory()) {
    fail(`admin dist must be a regular directory: ${root}`);
  }
  const files = [];
  const pending = [root];
  while (pending.length > 0) {
    const current = pending.pop();
    for (const name of readdirSync(current).sort().reverse()) {
      const item = path.join(current, name);
      const stats = lstatSync(item);
      if (stats.isSymbolicLink()) fail(`admin dist must not contain symlinks: ${item}`);
      if (stats.isDirectory()) {
        pending.push(item);
      } else if (stats.isFile()) {
        if (stats.size <= 0) fail(`admin dist contains an empty file: ${item}`);
        const bytes = readFileSync(item);
        files.push({
          path: path.relative(root, item).split(path.sep).join("/"),
          bytes: stats.size,
          sha256: sha256(bytes),
        });
      } else {
        fail(`admin dist contains an unsupported entry: ${item}`);
      }
    }
  }
  files.sort((left, right) => left.path.localeCompare(right.path));
  if (files.length === 0) fail("admin dist manifest is empty");
  return files;
}

function commandVersion(program, args, label) {
  const absolute = program.includes(path.sep) ? normalizePath(program) : program;
  try {
    const value = execFileSync(absolute, args, {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
    if (!value || value.length > 1024 || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(value)) {
      fail(`${label} returned an invalid version string`);
    }
    return value;
  } catch (error) {
    fail(`${label} version command failed: ${error.message}`);
  }
}

function sourceHashes() {
  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length === 0) {
    fail("execution contract required_source_files is empty");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => {
      const record = fileRecord(relativePath, `source file ${relativePath}`);
      return [relativePath, record.sha256];
    }),
  );
}

function writeAtomic(output, document) {
  const absolute = normalizePath(output);
  mkdirSync(path.dirname(absolute), { recursive: true });
  const temporary = `${absolute}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, absolute);
  return absolute;
}

const options = parseArguments(process.argv.slice(2));
if (!contract.build_snapshots?.profiles?.includes(options.profile)) {
  fail("--profile must be build-a or build-b");
}
for (const required of [
  "serverBinary",
  "trunk",
  "wasmBindgen",
  "commandLog",
  "output",
]) {
  if (!options[required]) fail(`--${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
}

const head = currentCommit();
const sourceCommit = options.sourceCommit
  ? requireCommit(options.sourceCommit, "--source-commit")
  : head;
if (sourceCommit !== head) {
  fail(`--source-commit ${sourceCommit} does not match git HEAD ${head}`);
}

const commandLog = fileRecord(options.commandLog, "build command log");
const artifacts = {
  embedded_admin_index: fileRecord(
    path.join(normalizePath(options.adminDist), "index.html"),
    "embedded admin index",
  ),
  embedded_admin_css: fileRecord(
    path.join(normalizePath(options.adminDist), "output.css"),
    "embedded admin stylesheet",
  ),
  authoring_bootstrap: fileRecord(options.bootstrap, "authoring bootstrap"),
  authoring_module: fileRecord(options.clientJs, "authoring JavaScript module"),
  authoring_wasm: fileRecord(options.clientWasm, "authoring WebAssembly module"),
  server_binary: fileRecord(options.serverBinary, "server binary", { executable: true }),
};

const document = {
  format: contract.build_snapshots.format,
  status: "passed",
  profile: options.profile,
  source_commit: sourceCommit,
  captured_at: new Date().toISOString(),
  toolchain: {
    node: process.version,
    cargo: commandVersion("cargo", ["--version"], "cargo"),
    rustc: commandVersion("rustc", ["--version", "--verbose"], "rustc"),
    trunk: commandVersion(options.trunk, ["--version"], "trunk"),
    wasm_bindgen: commandVersion(options.wasmBindgen, ["--version"], "wasm-bindgen"),
  },
  build_command_log: {
    bytes: commandLog.bytes,
    sha256: commandLog.sha256,
    raw_output_persisted: false,
  },
  source_sha256: sourceHashes(),
  artifacts,
  admin_dist_manifest: directoryManifest(options.adminDist),
  privacy: {
    raw_command_log_persisted: false,
    credentials_persisted: false,
    grants_or_proofs_persisted: false,
  },
};

const output = writeAtomic(options.output, document);
console.log(
  `[capture-pages-inline-edit-build-snapshot] PASS profile=${options.profile} output=${output}`,
);
