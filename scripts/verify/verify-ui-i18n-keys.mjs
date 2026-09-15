import fs from "node:fs";
import path from "node:path";

const workspaceRoot = process.cwd();
const modulesRoot = path.join(workspaceRoot, "crates", "modules");
const isStrict = process.argv.includes("--strict");

function readFtlKeys(filePath) {
  if (!fs.existsSync(filePath)) {
    return new Set();
  }
  const content = fs.readFileSync(filePath, "utf8");
  const keys = new Set();
  for (const line of content.split(/\r?\n/)) {
    const match = line.match(/^([a-zA-Z][a-zA-Z0-9_-]*)\s*=/);
    if (match) {
      keys.add(match[1]);
    }
  }
  return keys;
}

function findRsFiles(dir) {
  const results = [];
  if (!fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...findRsFiles(fullPath));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      results.push(fullPath);
    }
  }
  return results;
}

function extractKeyOccurrences(rsFilePath) {
  const rawContent = fs.readFileSync(rsFilePath, "utf8");
  // Strip out test modules to prevent false positives from mock calls or assertions
  const testModuleIndex = rawContent.indexOf("#[cfg(test)]");
  const content = testModuleIndex >= 0 ? rawContent.slice(0, testModuleIndex) : rawContent;
  const occurrences = [];

  // Match t(locale, "key", ...) or t!(..., "key", ...)
  const callRegex = /\bt\s*(?:!|\()\s*([^,\(\)]+|\([^)]*\))\s*,\s*"([a-zA-Z][a-zA-Z0-9_.-]+)"/gs;

  let match;
  while ((match = callRegex.exec(content)) !== null) {
    const rawKey = match[2];
    if (!rawKey.includes(".") && !rawKey.includes("-")) {
      continue;
    }

    const upToMatch = content.slice(0, match.index);
    const lineNum = upToMatch.split("\n").length;

    occurrences.push({
      line: lineNum,
      rawKey,
      kebabKey: rawKey.replaceAll(".", "-"),
    });
  }

  return occurrences;
}

function discoverModulePackages() {
  const packages = [];
  if (!fs.existsSync(modulesRoot)) return packages;

  const moduleDirs = fs.readdirSync(modulesRoot, { withFileTypes: true });
  for (const mod of moduleDirs) {
    if (!mod.isDirectory()) continue;
    const modPath = path.join(modulesRoot, mod.name);

    for (const surface of ["admin", "storefront"]) {
      const surfacePath = path.join(modPath, surface);
      const enFtlPath = path.join(surfacePath, "locales", "en.ftl");
      if (fs.existsSync(enFtlPath)) {
        packages.push({
          name: `${mod.name}/${surface}`,
          dir: surfacePath,
          enFtl: enFtlPath,
        });
      }
    }
  }

  return packages;
}

let hasError = false;
let totalCheckedKeys = 0;
let totalMissingKeys = 0;
const packages = discoverModulePackages();

for (const pkg of packages) {
  const ftlKeys = readFtlKeys(pkg.enFtl);
  const rsFiles = findRsFiles(path.join(pkg.dir, "src"));

  const missingInPkg = [];

  for (const rsFile of rsFiles) {
    if (rsFile.endsWith("i18n.rs")) continue;

    const occurrences = extractKeyOccurrences(rsFile);
    for (const occ of occurrences) {
      const isCandidateKey =
        occ.rawKey.includes(".") || occ.rawKey.includes("-");
      if (!isCandidateKey) continue;

      totalCheckedKeys++;
      if (!ftlKeys.has(occ.kebabKey) && !ftlKeys.has(occ.rawKey)) {
        missingInPkg.push({
          file: path.relative(workspaceRoot, rsFile),
          line: occ.line,
          key: occ.rawKey,
          kebab: occ.kebabKey,
        });
      }
    }
  }

  if (missingInPkg.length > 0) {
    hasError = true;
    totalMissingKeys += missingInPkg.length;
    const level = isStrict ? "FAIL" : "WARN";
    console.warn(`${level} ${pkg.name}: ${missingInPkg.length} missing key(s) in locales/en.ftl`);
    for (const item of missingInPkg.slice(0, 10)) {
      console.warn(
        `  ${item.file}:${item.line} - key "${item.key}" (lookup: "${item.kebab}")`,
      );
    }
    if (missingInPkg.length > 10) {
      console.warn(`  ... and ${missingInPkg.length - 10} more`);
    }
  } else {
    console.log(`OK   ${pkg.name} (${ftlKeys.size} catalog keys)`);
  }
}

console.log(
  `\nValidated ${totalCheckedKeys} UI key occurrences across ${packages.length} packages. (${totalMissingKeys} uncataloged key references)`,
);

if (isStrict && hasError) {
  process.exit(1);
}
