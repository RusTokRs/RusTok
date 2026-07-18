#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

function parseArguments(argv) {
  const options = { selfTest: false, rootPackage: "rustok-server" };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (
      [
        "--metadata",
        "--output",
        "--version",
        "--commit",
        "--repository",
        "--created-epoch",
        "--root-package",
      ].includes(argument)
    ) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function spdxId(packageId) {
  const digest = crypto.createHash("sha256").update(packageId).digest("hex").slice(0, 24);
  return `SPDXRef-Package-${digest}`;
}

function packagePurl(pkg) {
  return `pkg:cargo/${encodeURIComponent(pkg.name)}@${encodeURIComponent(pkg.version)}`;
}

function declaredLicense(pkg) {
  const value = typeof pkg.license === "string" ? pkg.license.trim() : "";
  return value || "NOASSERTION";
}

function packageEntry(pkg) {
  const entry = {
    SPDXID: spdxId(pkg.id),
    name: pkg.name,
    versionInfo: pkg.version,
    downloadLocation: "NOASSERTION",
    filesAnalyzed: false,
    licenseConcluded: "NOASSERTION",
    licenseDeclared: declaredLicense(pkg),
    copyrightText: "NOASSERTION",
    externalRefs: [
      {
        referenceCategory: "PACKAGE-MANAGER",
        referenceType: "purl",
        referenceLocator: packagePurl(pkg),
      },
    ],
  };
  if (typeof pkg.repository === "string" && pkg.repository.trim()) {
    entry.homepage = pkg.repository.trim();
  } else if (typeof pkg.homepage === "string" && pkg.homepage.trim()) {
    entry.homepage = pkg.homepage.trim();
  }
  if (typeof pkg.source === "string" && pkg.source.trim()) {
    entry.sourceInfo = `Cargo package source: ${pkg.source.trim()}`;
  } else {
    entry.sourceInfo = "Workspace package built from the release commit";
  }
  return entry;
}

function normalizeCreatedEpoch(raw) {
  if (!/^(?:0|[1-9]\d*)$/.test(String(raw || ""))) {
    throw new Error("--created-epoch must be a non-negative integer Unix timestamp");
  }
  const milliseconds = Number(raw) * 1000;
  if (!Number.isSafeInteger(milliseconds)) {
    throw new Error("--created-epoch is outside the supported timestamp range");
  }
  const created = new Date(milliseconds);
  if (!Number.isFinite(created.getTime())) throw new Error("--created-epoch is invalid");
  return created.toISOString().replace(".000Z", "Z");
}

function buildRelationships(metadata, packageIds, rootPackageId) {
  const relationships = [
    {
      spdxElementId: "SPDXRef-DOCUMENT",
      relationshipType: "DESCRIBES",
      relatedSpdxElement: packageIds.get(rootPackageId),
    },
  ];

  const nodes = metadata.resolve?.nodes;
  if (!Array.isArray(nodes)) {
    throw new Error("cargo metadata resolve.nodes must be present; do not use --no-deps");
  }
  for (const node of [...nodes].sort((left, right) => left.id.localeCompare(right.id))) {
    const sourceId = packageIds.get(node.id);
    if (!sourceId) throw new Error(`resolve node references unknown package ${node.id}`);
    const dependencies = [...new Set(node.dependencies || [])].sort();
    for (const dependency of dependencies) {
      const targetId = packageIds.get(dependency);
      if (!targetId) throw new Error(`resolve node ${node.id} references unknown dependency ${dependency}`);
      relationships.push({
        spdxElementId: sourceId,
        relationshipType: "DEPENDS_ON",
        relatedSpdxElement: targetId,
      });
    }
  }
  return relationships;
}

function generateSbom(metadata, options) {
  if (metadata.version !== 1) throw new Error("cargo metadata format version must be 1");
  if (!Array.isArray(metadata.packages) || metadata.packages.length === 0) {
    throw new Error("cargo metadata packages must be a non-empty array");
  }
  const packagesById = new Map();
  for (const pkg of metadata.packages) {
    if (!pkg?.id || !pkg?.name || !pkg?.version) {
      throw new Error("every cargo metadata package must include id, name and version");
    }
    if (packagesById.has(pkg.id)) throw new Error(`duplicate cargo metadata package id ${pkg.id}`);
    packagesById.set(pkg.id, pkg);
  }

  const rootCandidates = metadata.packages.filter((pkg) => pkg.name === options.rootPackage);
  if (rootCandidates.length !== 1) {
    throw new Error(
      `cargo metadata must contain exactly one ${options.rootPackage} package, found ${rootCandidates.length}`,
    );
  }
  const rootPackage = rootCandidates[0];
  if (rootPackage.version !== options.version) {
    throw new Error(
      `${options.rootPackage} metadata version ${rootPackage.version} does not match release ${options.version}`,
    );
  }

  const sortedPackages = [...metadata.packages].sort((left, right) => left.id.localeCompare(right.id));
  const packageIds = new Map(sortedPackages.map((pkg) => [pkg.id, spdxId(pkg.id)]));
  const created = normalizeCreatedEpoch(options.createdEpoch);
  const repository = options.repository.replace(/^https?:\/\/github\.com\//, "").replace(/\/$/, "");
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error("--repository must be owner/name or a canonical github.com repository URL");
  }
  if (!/^[0-9a-f]{40}$/i.test(options.commit)) {
    throw new Error("--commit must be a full 40-character Git commit SHA");
  }

  return {
    spdxVersion: "SPDX-2.3",
    dataLicense: "CC0-1.0",
    SPDXID: "SPDXRef-DOCUMENT",
    name: `RusToK ${options.version} server SBOM`,
    documentNamespace: `https://github.com/${repository}/releases/tag/v${encodeURIComponent(options.version)}/sbom/${options.commit}`,
    creationInfo: {
      created,
      creators: ["Tool: rustok-release-spdx-generator/1"],
      licenseListVersion: "3.27",
    },
    documentDescribes: [packageIds.get(rootPackage.id)],
    packages: sortedPackages.map(packageEntry),
    relationships: buildRelationships(metadata, packageIds, rootPackage.id),
  };
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function runSelfTest() {
  const rootId = "path+file:///repo/apps/server#rustok-server@1.2.3";
  const dependencyId = "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0";
  const metadata = {
    version: 1,
    packages: [
      {
        id: dependencyId,
        name: "serde",
        version: "1.0.0",
        source: "registry+https://github.com/rust-lang/crates.io-index",
        license: "MIT OR Apache-2.0",
      },
      {
        id: rootId,
        name: "rustok-server",
        version: "1.2.3",
        source: null,
        license: "BUSL-1.1",
        repository: "https://github.com/RusTokRs/RusTok",
      },
    ],
    resolve: {
      nodes: [
        { id: rootId, dependencies: [dependencyId] },
        { id: dependencyId, dependencies: [] },
      ],
    },
  };
  const options = {
    rootPackage: "rustok-server",
    version: "1.2.3",
    commit: "0123456789abcdef0123456789abcdef01234567",
    repository: "RusTokRs/RusTok",
    createdEpoch: "1784332800",
  };
  const first = stableJson(generateSbom(metadata, options));
  const second = stableJson(generateSbom(metadata, options));
  assert.equal(first, second);
  const parsed = JSON.parse(first);
  assert.equal(parsed.spdxVersion, "SPDX-2.3");
  assert.equal(parsed.packages.length, 2);
  assert.equal(parsed.relationships.filter((item) => item.relationshipType === "DEPENDS_ON").length, 1);
  assert.equal(parsed.creationInfo.created, "2026-07-18T00:00:00Z");
  assert.throws(
    () => generateSbom(metadata, { ...options, version: "1.2.4" }),
    /does not match release/,
  );
  console.log("✔ deterministic SPDX SBOM generator self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  for (const name of ["metadata", "output", "version", "commit", "repository", "createdEpoch"]) {
    if (!options[name]) throw new Error(`--${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
  }
  const metadata = JSON.parse(fs.readFileSync(path.resolve(options.metadata), "utf8"));
  const sbom = generateSbom(metadata, options);
  const output = path.resolve(options.output);
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, stableJson(sbom));
  console.log(`✔ wrote deterministic SPDX SBOM with ${sbom.packages.length} package(s) to ${output}`);
}

try {
  main();
} catch (error) {
  console.error(`SPDX SBOM generation failed: ${error.message}`);
  process.exit(1);
}
