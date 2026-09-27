import fs from "node:fs";
import path from "node:path";

const APPROVED = new Set([
  "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
  "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
  "actions/setup-node@820762786026740c76f36085b0efc47a31fe5020",
  "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
  "actions/download-artifact@37930b1c2abaa49bbe596cd826c3c89aef350131",
  "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
  "dtolnay/rust-toolchain@6bed0761d98439e5a578e2877258200ad565ba87",
  "dtolnay/rust-toolchain@02cb101ec7c40f2c49e1d9714d64511d8e1b74de",
  "Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6",
  "taiki-e/install-action@1ceecae37ba251f3f09906445d948ebfd5c4e06c",
  "taiki-e/install-action@38912a20a471c0a6a727cfdeb44492def5985f0c",
  "taiki-e/install-action@6db3a280da9eee4283c045c7431ab5116f1a0089",
  "taiki-e/install-action@23d41aa71228a2692ee31bb433f070fd4bdc6661",
  "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25",
  "crate-ci/typos@b41eb62e9a8096c9ba6636e2365d3f56018a203c",
  "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610",
]);

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
    if (reference.startsWith("./") || reference.startsWith("docker://")) return;
    if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+@[0-9a-f]{40}$/.test(reference)) {
      failures.push(`${file}:${index + 1}: mutable or malformed action reference: ${reference}`);
      return;
    }
    if (!APPROVED.has(reference)) {
      failures.push(`${file}:${index + 1}: unapproved action commit: ${reference}`);
    }
  });
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log("All third-party GitHub Actions are pinned to approved immutable commits.");
