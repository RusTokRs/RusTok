import fs from "node:fs";
import path from "node:path";

const root = path.resolve(
  process.env.RUSTOK_TAXONOMY_OWNERSHIP_ROOT || process.cwd(),
);
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const exists = (relative) => fs.existsSync(path.join(root, relative));

function walkRust(relative) {
  const absolute = path.join(root, relative);
  if (!fs.existsSync(absolute)) return [];
  const entries = fs.readdirSync(absolute, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const child = path.join(relative, entry.name);
    if (entry.isDirectory()) files.push(...walkRust(child));
    else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(child);
  }
  return files;
}

function requireMarkers(relative, markers) {
  if (!exists(relative)) {
    failures.push(`missing Taxonomy ownership artifact: ${relative}`);
    return;
  }
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing ownership marker: ${marker}`);
    }
  }
}

const taxonomySourceFiles = walkRust("crates/rustok-taxonomy/src");
if (taxonomySourceFiles.length === 0) {
  failures.push("Taxonomy production source tree is missing or empty");
}

const taxonomyPersistenceFiles = taxonomySourceFiles.filter(
  (relative) =>
    relative.includes("/src/entities/") || relative.includes("/src/migrations/"),
);

// Shared Category hierarchy and canonical presentation are accepted Taxonomy capabilities. What
// remains forbidden is moving consumer attachment/binding storage into Taxonomy or introducing an
// untyped polymorphic owner table. Blog/Forum/Product/Profile relations remain typed owner-module
// contracts.
const forbiddenConsumerRelations = [
  "blog_post_tags",
  "forum_topic_tags",
  "product_tags",
  "profile_tags",
  "BlogPostTags",
  "ForumTopicTags",
  "ProductTags",
  "ProfileTags",
];

for (const relative of taxonomyPersistenceFiles) {
  const source = read(relative);
  for (const marker of forbiddenConsumerRelations) {
    if (source.includes(marker)) {
      failures.push(
        `${relative}: consumer attachment storage must stay with its owning module; found ${marker}`,
      );
    }
  }
  if (/\bowner_type\b/.test(source) && /\bowner_id\b/.test(source)) {
    failures.push(
      `${relative}: Taxonomy persistence must not introduce generic polymorphic owner_type/owner_id attachment storage`,
    );
  }
}

requireMarkers(
  "DECISIONS/2026-08-22-taxonomy-category-flex-ownership.md",
  [
    "`rustok-taxonomy` becomes the canonical Category owner",
    "Taxonomy does not",
    "generic polymorphic `owner_type/owner_id` attachment table",
  ],
);
requireMarkers(
  "docs/architecture/taxonomy-flex-category-platform-plan.md",
  [
    "Taxonomy owns shared Category hierarchy",
    "canonical presentation such as icon key, color and Media-owned image/cover references",
    "Consumer relation/binding tables stay with the consumer",
    "Flex is the only runtime custom-fields mechanism",
  ],
);

requireMarkers(
  "crates/rustok-blog/src/migrations/m20260328_000002_create_blog_taxonomy_tables.rs",
  [
    ".table(BlogPostTags::Table)",
    ".to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
  ],
);
requireMarkers("crates/rustok-blog/src/entities/blog_post_tag.rs", [
  'table_name = "blog_post_tags"',
]);

requireMarkers(
  "crates/rustok-forum/src/migrations/m20260329_000005_create_forum_topic_tags.rs",
  [
    ".table(ForumTopicTags::Table)",
    "ForumTopicTags::TopicId",
    "ForumTopicTags::TermId",
    ".to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
  ],
);
requireMarkers("crates/rustok-forum/src/entities/forum_topic_tag.rs", [
  'table_name = "forum_topic_tags"',
]);

requireMarkers(
  "crates/rustok-product/src/migrations/m20260329_000001_create_product_tags.rs",
  [
    ".table(ProductTags::Table)",
    "ProductTags::ProductId",
    "ProductTags::TermId",
    ".to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
  ],
);
requireMarkers("crates/rustok-product/src/entities/product_tag.rs", [
  'table_name = "product_tags"',
]);

requireMarkers(
  "crates/rustok-profiles/src/migrations/m20260330_000002_create_profile_tags.rs",
  [
    ".table(ProfileTags::Table)",
    "ProfileTags::ProfileUserId",
    "ProfileTags::TermId",
    ".to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
  ],
);
requireMarkers("crates/rustok-profiles/src/entities/profile_tag.rs", [
  'table_name = "profile_tags"',
]);

if (failures.length > 0) {
  console.error("Taxonomy ownership boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Taxonomy ownership boundary checks passed: ${taxonomySourceFiles.length} Taxonomy Rust source files may own shared Category hierarchy/presentation; Blog, Forum, Product, and Profiles retain typed consumer attachment relations and generic polymorphic attachment storage remains forbidden.`,
);
