import fs from "node:fs";
import path from "node:path";

const workspaceRoot = process.cwd();
const scanRoots = ["apps", "crates", "packages"];
const bundleDirs = new Set(["locales", "messages"]);
const excludedPathFragments = [
  `${path.sep}crates${path.sep}rustok-commerce`,
  `${path.sep}crates${path.sep}rustok-commerce-foundation`,
  `${path.sep}crates${path.sep}rustok-cart`,
  `${path.sep}crates${path.sep}rustok-customer`,
  `${path.sep}crates${path.sep}rustok-product`,
  `${path.sep}crates${path.sep}rustok-region`,
  `${path.sep}crates${path.sep}rustok-pricing`,
  `${path.sep}crates${path.sep}rustok-inventory`,
  `${path.sep}crates${path.sep}rustok-order`,
  `${path.sep}crates${path.sep}rustok-payment`,
  `${path.sep}crates${path.sep}rustok-fulfillment`,
  `${path.sep}apps${path.sep}next-admin${path.sep}src${path.sep}features${path.sep}products`,
];

function isExcluded(targetPath) {
  return excludedPathFragments.some((fragment) =>
    targetPath.includes(fragment),
  );
}

function normalizeLocaleTag(value) {
  const normalized = value.trim().replaceAll("_", "-");
  if (!normalized || normalized.length > 32) {
    return null;
  }

  const parts = normalized
    .split("-")
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length === 0) {
    return null;
  }

  const rebuilt = [];
  for (const [index, part] of parts.entries()) {
    if (!/^[A-Za-z0-9]+$/.test(part)) {
      return null;
    }

    if (index === 0) {
      rebuilt.push(part.toLowerCase());
      continue;
    }

    if (/^[A-Za-z]{2}$/.test(part)) {
      rebuilt.push(part.toUpperCase());
      continue;
    }

    if (/^[A-Za-z]{4}$/.test(part)) {
      rebuilt.push(`${part[0].toUpperCase()}${part.slice(1).toLowerCase()}`);
      continue;
    }

    if (/^\d{3}$/.test(part)) {
      rebuilt.push(part);
      continue;
    }

    rebuilt.push(part.toLowerCase());
  }

  return rebuilt.join("-");
}

function flattenJson(value, prefix = "") {
  if (Array.isArray(value)) {
    return value.flatMap((item, index) =>
      flattenJson(item, `${prefix}[${index}]`),
    );
  }

  if (value && typeof value === "object") {
    return Object.entries(value).flatMap(([key, child]) => {
      const nextPrefix = prefix ? `${prefix}.${key}` : key;
      return flattenJson(child, nextPrefix);
    });
  }

  return [prefix];
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function walkDirectories(rootPath, onDirectory) {
  if (!fs.existsSync(rootPath) || isExcluded(rootPath)) {
    return;
  }

  const stack = [rootPath];
  while (stack.length > 0) {
    const current = stack.pop();
    if (isExcluded(current)) {
      continue;
    }

    const entries = fs.readdirSync(current, { withFileTypes: true });
    onDirectory(current, entries);

    for (const entry of entries) {
      if (!entry.isDirectory()) {
        continue;
      }
      if (
        entry.name === "node_modules" ||
        entry.name === "target" ||
        entry.name.startsWith(".")
      ) {
        continue;
      }
      stack.push(path.join(current, entry.name));
    }
  }
}

function discoverBundleDirs() {
  const results = [];

  for (const relativeRoot of scanRoots) {
    const absoluteRoot = path.join(workspaceRoot, relativeRoot);
    walkDirectories(absoluteRoot, (directory, entries) => {
      if (isExcluded(directory) || !bundleDirs.has(path.basename(directory))) {
        return;
      }

      const jsonFiles = entries
        .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
        .map((entry) => entry.name)
        .sort((left, right) => left.localeCompare(right));

      if (jsonFiles.length < 2) {
        return;
      }

      results.push({
        directory,
        files: jsonFiles.map((fileName) => path.join(directory, fileName)),
      });
    });
  }

  return results.sort((left, right) =>
    left.directory.localeCompare(right.directory),
  );
}

function compareBundleDirectory(bundleDirectory) {
  const files = bundleDirectory.files.map((filePath) => ({
    filePath,
    fileName: path.basename(filePath),
    locale: path.basename(filePath, ".json"),
    keys: new Set(flattenJson(readJson(filePath)).filter(Boolean)),
  }));

  const invalidLocaleFiles = files
    .filter(({ locale }) => normalizeLocaleTag(locale) !== locale)
    .map(({ fileName }) => fileName);

  const [baseline, ...others] = files;
  const mismatches = [];

  for (const current of others) {
    const missingOnCurrent = [...baseline.keys]
      .filter((key) => !current.keys.has(key))
      .sort();
    const missingOnBaseline = [...current.keys]
      .filter((key) => !baseline.keys.has(key))
      .sort();

    if (missingOnCurrent.length > 0 || missingOnBaseline.length > 0) {
      mismatches.push({
        baseline: baseline.fileName,
        current: current.fileName,
        missingOnCurrent,
        missingOnBaseline,
      });
    }
  }

  return { invalidLocaleFiles, mismatches };
}

const bundleDirsToCheck = discoverBundleDirs();

if (bundleDirsToCheck.length === 0) {
  console.error("No UI locale/message bundle directories found.");
  process.exit(1);
}

let hasMismatch = false;

for (const bundleDirectory of bundleDirsToCheck) {
  const result = compareBundleDirectory(bundleDirectory);
  const relativeDirectory = path.relative(
    workspaceRoot,
    bundleDirectory.directory,
  );

  if (
    result.invalidLocaleFiles.length === 0 &&
    result.mismatches.length === 0
  ) {
    console.log(`OK  ${relativeDirectory}`);
    continue;
  }

  hasMismatch = true;
  console.error(`FAIL ${relativeDirectory}`);

  if (result.invalidLocaleFiles.length > 0) {
    console.error(
      `  Invalid locale bundle filenames: ${result.invalidLocaleFiles.join(", ")}`,
    );
  }

  for (const mismatch of result.mismatches) {
    if (mismatch.missingOnCurrent.length > 0) {
      console.error(
        `  Missing in ${mismatch.current} vs ${mismatch.baseline}: ${mismatch.missingOnCurrent.join(", ")}`,
      );
    }
    if (mismatch.missingOnBaseline.length > 0) {
      console.error(
        `  Missing in ${mismatch.baseline} vs ${mismatch.current}: ${mismatch.missingOnBaseline.join(", ")}`,
      );
    }
  }
}

if (hasMismatch) {
  process.exit(1);
}
