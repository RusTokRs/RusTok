import fs from "node:fs";
import path from "node:path";

const workspaceRoot = process.cwd();
const cratesRoot = path.join(workspaceRoot, "crates");
const hardcodedModuleRoutePattern = /"\/modules\//;

function walkRustFiles(rootPath) {
  if (!fs.existsSync(rootPath)) {
    return [];
  }

  const files = [];
  const stack = [rootPath];

  while (stack.length > 0) {
    const current = stack.pop();
    const entries = fs.readdirSync(current, { withFileTypes: true });

    for (const entry of entries) {
      const absolutePath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "target" || entry.name === "node_modules" || entry.name.startsWith(".")) {
          continue;
        }
        stack.push(absolutePath);
        continue;
      }

      if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(absolutePath);
      }
    }
  }

  return files.sort((left, right) => left.localeCompare(right));
}

function discoverStorefrontRustFiles() {
  if (!fs.existsSync(cratesRoot)) {
    return [];
  }

  return fs
    .readdirSync(cratesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((entry) => {
      const storefrontSrc = path.join(cratesRoot, entry.name, "storefront", "src");
      return walkRustFiles(storefrontSrc);
    });
}

const violations = [];

for (const filePath of discoverStorefrontRustFiles()) {
  const content = fs.readFileSync(filePath, "utf8");
  const lines = content.split(/\r?\n/);

  for (const [index, line] of lines.entries()) {
    if (!hardcodedModuleRoutePattern.test(line)) {
      continue;
    }

    violations.push({
      filePath,
      lineNumber: index + 1,
      line: line.trim(),
    });
  }
}

if (violations.length === 0) {
  console.log("OK  module-owned storefront packages use locale-aware route helpers");
  process.exit(0);
}

console.error("FAIL module-owned storefront packages contain hardcoded /modules/ routes");

for (const violation of violations) {
  const relativePath = path.relative(workspaceRoot, violation.filePath);
  console.error(`  ${relativePath}:${violation.lineNumber}`);
  console.error(`    ${violation.line}`);
}

console.error("Use UiRouteContext::module_route_base() instead of hardcoded /modules/... links.");
process.exit(1);
