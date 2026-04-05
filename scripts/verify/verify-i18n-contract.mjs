import fs from "node:fs";
import path from "node:path";

const workspaceRoot = process.cwd();
const failures = [];

function readWorkspaceFile(relativePath) {
  return fs.readFileSync(path.join(workspaceRoot, relativePath), "utf8");
}

function expectContains(relativePath, expectedSnippet, description) {
  const content = readWorkspaceFile(relativePath);
  if (!content.includes(expectedSnippet)) {
    failures.push(`${relativePath}: expected ${description}`);
  }
}

function expectNotContains(relativePath, unexpectedSnippet, description) {
  const content = readWorkspaceFile(relativePath);
  if (content.includes(unexpectedSnippet)) {
    failures.push(`${relativePath}: found ${description}`);
  }
}

function walkDirectory(relativeRoot, visitor) {
  const absoluteRoot = path.join(workspaceRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) {
    return;
  }

  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    const entries = fs.readdirSync(current, { withFileTypes: true });

    for (const entry of entries) {
      const absolutePath = path.join(current, entry.name);
      const relativePath = path.relative(workspaceRoot, absolutePath);

      if (entry.isDirectory()) {
        if (
          entry.name === "node_modules" ||
          entry.name === "target" ||
          entry.name.startsWith(".") ||
          relativePath ===
            path.join("apps", "next-admin", "src", "features", "products")
        ) {
          continue;
        }
        stack.push(absolutePath);
        continue;
      }

      visitor({ absolutePath, relativePath });
    }
  }
}

expectContains(
  "apps/next-frontend/src/i18n.ts",
  'export const defaultLocale = "en";',
  "apps/next-frontend to use platform fallback locale 'en'",
);
expectContains(
  "apps/next-admin/src/i18n/request.ts",
  "export const defaultLocale: Locale = 'en';",
  "apps/next-admin to use platform fallback locale 'en'",
);
expectContains(
  "apps/admin/src/shared/i18n/mod.rs",
  "load_locale_from_storage().unwrap_or(Locale::En)",
  "Leptos admin locale context to default to Locale::En",
);
expectNotContains(
  "apps/admin/src/shared/i18n/mod.rs",
  "_ => Locale::Ru",
  "legacy Locale::Ru fallback in Locale::from_code",
);
expectContains(
  "apps/server/src/modules/manifest.rs",
  "normalize_locale_tag(locale)",
  "manifest i18n validation to normalize locale tags via rustok-core",
);
expectNotContains(
  "apps/server/src/modules/manifest.rs",
  "is_valid_locale_key(",
  "legacy short-form locale validator in manifest i18n contract",
);
expectNotContains(
  "crates/rustok-core/src/field_schema.rs",
  "LOCALE_KEY_REGEX",
  "legacy locale regex in rustok-core field schema",
);
expectNotContains(
  "crates/rustok-ai/src/metrics.rs",
  "fn locale_tags_match(",
  "duplicate locale matcher in rustok-ai metrics",
);
expectNotContains(
  "crates/rustok-ai/src/metrics.rs",
  "fn normalize_locale_tag(",
  "duplicate locale normalizer in rustok-ai metrics",
);

const forbiddenLocaleDefaultPatterns = [
  "default('en')",
  "locale: 'en'",
  "|| 'en'",
  "?? 'en'",
];

walkDirectory(
  path.join("apps", "next-admin", "src", "features"),
  ({ absolutePath, relativePath }) => {
    if (!absolutePath.endsWith(".ts") && !absolutePath.endsWith(".tsx")) {
      return;
    }

    const content = fs.readFileSync(absolutePath, "utf8");
    for (const pattern of forbiddenLocaleDefaultPatterns) {
      if (content.includes(pattern)) {
        failures.push(
          `${relativePath}: found forbidden locale default pattern ${JSON.stringify(pattern)}`,
        );
      }
    }
  },
);

if (failures.length > 0) {
  console.error("i18n contract drift detected:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("OK  i18n contract");
