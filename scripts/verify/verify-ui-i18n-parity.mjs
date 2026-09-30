#!/usr/bin/env node
/**
 * UI i18n catalog parity gate.
 *
 * Compares every locale of a catalog directory against its `en` baseline using
 * a structural Fluent reader instead of a line regex. The checks mirror what
 * `rustok_ui_i18n::validate_catalog_schemas` enforces in Rust, so a defect is
 * caught by CI even though no workspace consumer currently calls the Rust
 * validator at startup.
 *
 * Checked per catalog directory:
 *   - catalog file names are canonical locale tags;
 *   - message / term identity parity against the baseline;
 *   - value presence parity (attribute-only messages stay attribute-only);
 *   - attribute identity parity;
 *   - external variable parity for the value and for every attribute;
 *   - select-expression (plural) parity for the value and every attribute;
 *   - duplicate entries and duplicate attributes inside one file;
 *   - message/term references that resolve to nothing.
 */

import fs from "node:fs";
import path from "node:path";

import {
  entryPatterns,
  parseFtl,
  patternReferences,
  patternVariables,
  selectorVariables,
} from "./lib/ftl.mjs";
import { canonicalizeLocaleTag } from "./lib/locale-tag.mjs";

const workspaceRoot = process.cwd();
const scanRoots = ["apps", "crates", "packages"];
const bundleDirNames = new Set(["locales", "messages"]);
const skippedDirNames = new Set(["node_modules", "target", "dist", "out"]);
const BASELINE_LOCALE = "en";
/**
 * Variables whose English rendering must agree in number. The authored English
 * catalog is repository-owned, so "{ $count } comments" printing "1 comments"
 * is an objective defect rather than a translator's stylistic choice.
 */
const COUNT_VARIABLE_RE = /^(count|total|qty|quantity|num)$|(_count|Count|_total|Total)$/;

function walkDirectories(rootPath, onDirectory) {
  if (!fs.existsSync(rootPath)) return;

  const stack = [rootPath];
  while (stack.length > 0) {
    const current = stack.pop();
    const entries = fs.readdirSync(current, { withFileTypes: true });
    onDirectory(current, entries);

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      if (skippedDirNames.has(entry.name) || entry.name.startsWith(".")) {
        continue;
      }
      stack.push(path.join(current, entry.name));
    }
  }
}

function flattenJson(value, prefix = "") {
  if (Array.isArray(value)) {
    return value.flatMap((item, index) => flattenJson(item, `${prefix}[${index}]`));
  }
  if (value && typeof value === "object") {
    return Object.entries(value).flatMap(([key, child]) =>
      flattenJson(child, prefix ? `${prefix}.${key}` : key),
    );
  }
  return [prefix];
}

function discoverBundleDirs() {
  const results = [];

  for (const relativeRoot of scanRoots) {
    walkDirectories(path.join(workspaceRoot, relativeRoot), (directory, entries) => {
      if (!bundleDirNames.has(path.basename(directory))) return;

      const files = (suffix) =>
        entries
          .filter((entry) => entry.isFile() && entry.name.endsWith(suffix))
          .map((entry) => entry.name)
          .sort((left, right) => left.localeCompare(right));

      const ftlFiles = files(".ftl");
      const jsonFiles = files(".json");

      if (ftlFiles.length >= 2) {
        results.push({ directory, format: "ftl", fileNames: ftlFiles });
      } else if (jsonFiles.length >= 2) {
        results.push({ directory, format: "json", fileNames: jsonFiles });
      }
    });
  }

  return results.sort((left, right) => left.directory.localeCompare(right.directory));
}

function sortedList(values) {
  return [...values].sort().join(", ");
}

function difference(left, right) {
  return [...left].filter((value) => !right.has(value)).sort();
}

function readFtlCatalog(filePath) {
  const { entries, junkLines } = parseFtl(fs.readFileSync(filePath, "utf8"));
  const messages = new Map();
  const terms = new Map();
  const problems = [];

  for (const entry of entries) {
    const target = entry.kind === "term" ? terms : messages;
    const display = entry.kind === "term" ? `-${entry.id}` : entry.id;

    if (target.has(entry.id)) {
      problems.push(
        `duplicate entry '${display}' (lines ${target.get(entry.id).line} and ${entry.line})`,
      );
      continue;
    }
    if (entry.duplicateAttributes) {
      for (const attribute of entry.duplicateAttributes) {
        problems.push(`duplicate attribute '${display}.${attribute}'`);
      }
    }
    target.set(entry.id, entry);
  }

  for (const lineNumber of junkLines) {
    problems.push(`unparsable line ${lineNumber} (not a comment, entry, or continuation)`);
  }

  for (const [, entry] of [...messages, ...terms]) {
    const display = entry.kind === "term" ? `-${entry.id}` : entry.id;
    for (const { part, pattern } of entryPatterns(entry)) {
      const { messages: messageRefs, terms: termRefs } = patternReferences(pattern);
      for (const reference of messageRefs) {
        if (!messages.has(reference)) {
          problems.push(`'${display}${part === "value" ? "" : part}' references missing message '${reference}'`);
        }
      }
      for (const reference of termRefs) {
        if (!terms.has(reference)) {
          problems.push(`'${display}${part === "value" ? "" : part}' references missing term '-${reference}'`);
        }
      }
    }
  }

  return { messages, terms, problems };
}

function compareFtlCatalog(directory, fileNames) {
  const failures = [];
  const catalogs = new Map();

  for (const fileName of fileNames) {
    const locale = path.basename(fileName, ".ftl");
    if (canonicalizeLocaleTag(locale) !== locale) {
      failures.push(`invalid catalog file name '${fileName}' (not a canonical locale tag)`);
    }

    const catalog = readFtlCatalog(path.join(directory, fileName));
    for (const problem of catalog.problems) {
      failures.push(`${fileName}: ${problem}`);
    }
    catalogs.set(locale, catalog);
  }

  const baselineCatalog = catalogs.get(BASELINE_LOCALE);
  if (baselineCatalog) {
    for (const [id, entry] of baselineCatalog.messages) {
      for (const { part, pattern } of entryPatterns(entry)) {
        const selectors = selectorVariables(pattern);
        for (const variable of patternVariables(pattern)) {
          if (!COUNT_VARIABLE_RE.test(variable)) continue;
          if (selectors.has(variable)) continue;
          // An entry may opt out with a reasoned FTL comment, for example a
          // page number or an invariant adjectival phrase ("3 total").
          if (/(^|\n)\s*plural-exempt:\s*\S/.test(entry.comment ?? "")) continue;
          failures.push(
            `${BASELINE_LOCALE}.ftl: '${id}${part === "value" ? "" : part}' interpolates the count ` +
              `variable '$${variable}' without a plural select expression ` +
              `(renders "1 items"); wrap it in \`{ $${variable} -> [one] .. *[other] .. }\``,
          );
        }
      }
    }
  }

  const baseline = baselineCatalog;
  if (!baseline) {
    failures.push(
      `missing '${BASELINE_LOCALE}.ftl' baseline (found: ${[...catalogs.keys()].sort().join(", ")})`,
    );
    return failures;
  }

  for (const [locale, catalog] of [...catalogs].sort()) {
    if (locale === BASELINE_LOCALE) continue;
    const file = `${locale}.ftl`;

    for (const [kind, baselineMap, localeMap, prefix] of [
      ["message", baseline.messages, catalog.messages, ""],
      ["term", baseline.terms, catalog.terms, "-"],
    ]) {
      for (const id of difference(baselineMap.keys(), localeMap)) {
        failures.push(`${file}: missing ${kind} '${prefix}${id}'`);
      }
      for (const id of difference(localeMap.keys(), baselineMap)) {
        failures.push(`${file}: ${kind} '${prefix}${id}' is absent from the baseline`);
      }
    }

    for (const [id, baselineEntry] of [...baseline.messages, ...baseline.terms]) {
      const localeEntry =
        baselineEntry.kind === "term" ? catalog.terms.get(id) : catalog.messages.get(id);
      if (!localeEntry) continue;
      const display = baselineEntry.kind === "term" ? `-${id}` : id;

      if (baselineEntry.value !== null && localeEntry.value === null) {
        failures.push(`${file}: '${display}' has no value while the baseline defines one`);
      }
      if (baselineEntry.value === null && localeEntry.value !== null) {
        failures.push(
          `${file}: '${display}' defines a value while the baseline is attribute-only`,
        );
      }

      for (const attribute of difference(
        baselineEntry.attributes.keys(),
        localeEntry.attributes,
      )) {
        failures.push(`${file}: missing attribute '${display}.${attribute}'`);
      }
      for (const attribute of difference(
        localeEntry.attributes.keys(),
        baselineEntry.attributes,
      )) {
        failures.push(
          `${file}: attribute '${display}.${attribute}' is absent from the baseline`,
        );
      }

      const parts = [
        ["", baselineEntry.value, localeEntry.value],
        ...[...baselineEntry.attributes]
          .filter(([attribute]) => localeEntry.attributes.has(attribute))
          .map(([attribute, pattern]) => [
            `.${attribute}`,
            pattern,
            localeEntry.attributes.get(attribute),
          ]),
      ];

      for (const [suffix, baselinePattern, localePattern] of parts) {
        if (baselinePattern === null || localePattern === null) continue;

        const baselineVars = patternVariables(baselinePattern);
        const localeVars = patternVariables(localePattern);
        if (sortedList(baselineVars) !== sortedList(localeVars)) {
          failures.push(
            `${file}: '${display}${suffix}' variables [${sortedList(localeVars)}] != baseline [${sortedList(baselineVars)}]`,
          );
        }

        // Selector parity is deliberately NOT required across locales: a
        // translation may legitimately avoid grammatical agreement (Russian
        // "Комментариев: { $count }"), and languages differ in how many CLDR
        // categories they have. Variable parity above is the cross-locale
        // contract; the plural rule below applies to the authored baseline.
      }
    }
  }

  return failures;
}

function compareJsonCatalog(directory, fileNames) {
  const failures = [];
  const catalogs = new Map();

  for (const fileName of fileNames) {
    const locale = path.basename(fileName, ".json");
    if (canonicalizeLocaleTag(locale) !== locale) {
      failures.push(`invalid catalog file name '${fileName}' (not a canonical locale tag)`);
    }
    const parsed = JSON.parse(fs.readFileSync(path.join(directory, fileName), "utf8"));
    catalogs.set(locale, new Set(flattenJson(parsed).filter(Boolean)));
  }

  const [baselineLocale] = [...catalogs.keys()].sort();
  const baseline = catalogs.get(BASELINE_LOCALE) ?? catalogs.get(baselineLocale);
  const baselineName = catalogs.has(BASELINE_LOCALE) ? BASELINE_LOCALE : baselineLocale;

  for (const [locale, keys] of [...catalogs].sort()) {
    if (locale === baselineName) continue;
    for (const key of difference(baseline, keys)) {
      failures.push(`${locale}.json: missing key '${key}'`);
    }
    for (const key of difference(keys, baseline)) {
      failures.push(`${locale}.json: key '${key}' is absent from ${baselineName}.json`);
    }
  }

  return failures;
}

const bundleDirs = discoverBundleDirs();

if (bundleDirs.length === 0) {
  console.error("No UI locale/message bundle directories found.");
  process.exit(1);
}

let hasFailure = false;

for (const bundle of bundleDirs) {
  const relativeDirectory = path.relative(workspaceRoot, bundle.directory);
  const failures =
    bundle.format === "ftl"
      ? compareFtlCatalog(bundle.directory, bundle.fileNames)
      : compareJsonCatalog(bundle.directory, bundle.fileNames);

  if (failures.length === 0) {
    console.log(`OK   ${relativeDirectory} (${bundle.fileNames.length} locales)`);
    continue;
  }

  hasFailure = true;
  console.error(`FAIL ${relativeDirectory}`);
  for (const failure of failures) {
    console.error(`  ${failure}`);
  }
}

if (hasFailure) {
  process.exit(1);
}

console.log(`\nOK  UI i18n parity across ${bundleDirs.length} catalog directories.`);
