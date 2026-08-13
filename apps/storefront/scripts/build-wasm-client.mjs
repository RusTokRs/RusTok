import { cpSync, mkdirSync, readFileSync, renameSync, rmSync, statSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..");

export function buildStorefrontWasmClient(config) {
  const profile = process.env[config.profileEnv]?.trim() || "release";
  if (!new Set(["debug", "release"]).has(profile)) {
    throw new Error(`${config.profileEnv} must be \`debug\` or \`release\``);
  }

  const lockedWasmBindgenVersion = lockedPackageVersion("wasm-bindgen");
  if (process.argv.length === 3 && process.argv[2] === "--print-wasm-bindgen-version") {
    process.stdout.write(`${lockedWasmBindgenVersion}\n`);
    return;
  }
  if (process.argv.length !== 2) throw new Error(config.usage);

  const cargoTargetRoot = process.env.CARGO_TARGET_DIR?.trim()
    ? path.resolve(repoRoot, process.env.CARGO_TARGET_DIR.trim())
    : path.join(repoRoot, "target");
  const targetRoot = path.resolve(
    repoRoot,
    process.env[config.assetDirEnv]?.trim() || config.defaultAssetDir,
  );
  const assetRoot = path.dirname(targetRoot);
  const stagingRoot = `${targetRoot}.tmp-${process.pid}`;
  const wasmBindgen = process.env.RUSTOK_WASM_BINDGEN_BIN?.trim() || "wasm-bindgen";
  const cargoArgs = [
    "build",
    "--locked",
    "-p",
    "rustok-storefront",
    "--lib",
    "--target",
    "wasm32-unknown-unknown",
    "--no-default-features",
    "--features",
    config.feature,
  ];
  if (profile === "release") cargoArgs.push("--release");

  const wasmBindgenVersion = run(wasmBindgen, ["--version"], true);
  if (wasmBindgenVersion !== `wasm-bindgen ${lockedWasmBindgenVersion}`) {
    throw new Error(
      `expected wasm-bindgen ${lockedWasmBindgenVersion}, found ${wasmBindgenVersion || "no version"}`,
    );
  }

  rmSync(stagingRoot, { force: true, recursive: true });
  mkdirSync(stagingRoot, { recursive: true });
  run("cargo", cargoArgs);

  const wasmInput = path.join(
    cargoTargetRoot,
    "wasm32-unknown-unknown",
    profile,
    "rustok_storefront.wasm",
  );
  requireNonEmptyFile(wasmInput, `compiled ${config.label} WASM input`);
  run(wasmBindgen, [
    wasmInput,
    "--target",
    "web",
    "--out-dir",
    stagingRoot,
    "--out-name",
    "rustok_storefront",
    "--no-typescript",
  ]);

  for (const file of ["rustok_storefront.js", "rustok_storefront_bg.wasm"]) {
    requireNonEmptyFile(path.join(stagingRoot, file), `generated ${file}`);
  }
  rmSync(targetRoot, { force: true, recursive: true });
  mkdirSync(assetRoot, { recursive: true });
  renameSync(stagingRoot, targetRoot);
  cpSync(
    path.join(repoRoot, config.bootstrapSource),
    path.join(assetRoot, config.bootstrapTarget),
  );
  requireNonEmptyFile(path.join(assetRoot, config.bootstrapTarget), `${config.label} bootstrap asset`);
  console.log(
    `[${config.logPrefix}] locked_wasm_bindgen=${lockedWasmBindgenVersion} assets=${targetRoot}`,
  );
}

function lockedPackageVersion(packageName) {
  const lock = readFileSync(path.join(repoRoot, "Cargo.lock"), "utf8");
  const versions = [...new Set(lock.split(/^\[\[package\]\]\s*$/m).flatMap((block) => {
    const name = block.match(/^name = "([^"]+)"$/m)?.[1];
    const version = block.match(/^version = "([^"]+)"$/m)?.[1];
    return name === packageName && version ? [version] : [];
  }))];
  if (versions.length !== 1) {
    throw new Error(
      `Cargo.lock must contain exactly one ${packageName} version, found ${versions.join(", ") || "none"}`,
    );
  }
  return versions[0];
}

function run(command, args, capture = false) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    encoding: capture ? "utf8" : undefined,
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const details = capture ? `: ${(result.stderr || result.stdout || "").trim()}` : "";
    throw new Error(`${command} exited with status ${result.status}${details}`);
  }
  return capture ? (result.stdout || "").trim() : "";
}

function requireNonEmptyFile(file, label) {
  let stats;
  try {
    stats = statSync(file);
  } catch (error) {
    throw new Error(`${label} is missing at ${file}: ${error.message}`);
  }
  if (!stats.isFile() || stats.size === 0) {
    throw new Error(`${label} must be a non-empty regular file: ${file}`);
  }
}
