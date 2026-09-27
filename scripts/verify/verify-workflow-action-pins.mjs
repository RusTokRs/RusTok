import fs from "node:fs";
import path from "node:path";

const USES = /^\s*(?:-\s*)?uses:\s*([^\s#]+)(?:\s+#.*)?$/;

function walk(directory) {
  const files = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(full));
    else if (/\.ya?ml$/.test(entry.name)) files.push(full);
  }
  return files;
}

const failures = [];

for (const file of walk(path.join(process.cwd(), ".github", "workflows"))) {
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    const match = line.match(USES);
    if (!match) return;

    const reference = match[1];

    if (reference.startsWith("./") || reference.startsWith("docker://")) {
      return;
    }

    if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+@[0-9a-f]{40}$/.test(reference)) {
      failures.push(
        `${file}:${index + 1}: mutable or malformed third-party action reference: ${reference}`,
      );
    }
  });
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log("All third-party GitHub Actions are pinned to immutable commit SHAs.");
