import fs from "node:fs";
import path from "node:path";

const USES = /^\s*(?:-\s*)?uses:\s*([^\s#]+)(?:\s+#.*)?$/;

// `dtolnay/rust-toolchain` is pinned to a commit whose `action.yml` declares `toolchain` as
// required with no default, so a step that omits the input dies with
// "'toolchain' is a required input" before anything is installed. The action's per-toolchain
// branches carry a `default: stable`, but the pinned commit does not, and an explicit version also
// keeps CI on the version the workspace declares instead of the drifting `stable` channel.
const RUST_TOOLCHAIN = /^dtolnay\/rust-toolchain@[0-9a-f]{40}$/;

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

function stepInput(lines, usesIndex, input) {
  const usesIndent = lines[usesIndex].search(/\S/);
  for (let index = usesIndex + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === "") continue;
    const indent = line.search(/\S/);
    if (indent <= usesIndent) return false;
    if (new RegExp(`^\\s*${input}:`).test(line)) return true;
  }
  return false;
}

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

    if (RUST_TOOLCHAIN.test(reference) && !stepInput(lines, index, "toolchain")) {
      failures.push(
        `${file}:${index + 1}: dtolnay/rust-toolchain step must declare an explicit toolchain input`,
      );
    }
  });
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log("All third-party GitHub Actions are pinned to immutable commit SHAs.");
