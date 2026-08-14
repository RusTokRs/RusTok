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
    failures.push(`missing owner-owned Taxonomy consumer artifact: ${relative}`);
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

const forbiddenHierarchy = [
  { pattern: /\bparent_id\b/, label: "generic parent_id" },
  { pattern: /\bParentId\b/, label: "generic ParentId identifier" },
  { pattern: /\bcategory_parent\b/i, label: "category-parent storage" },
  { pattern: /\bcategory_closure\b/i, label: "category closure storage" },
];

for (const relative of taxonomySourceFiles) {
  const source = read(relative);
  for (const { pattern, label } of forbiddenHierarchy) {
    if (pattern.test(source)) {
      failures.push(
        `${relative}: Taxonomy must remain a flat vocabulary layer; found ${label}`,
      );
    }
  }
}

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
  "crates/rustok-blog/src/migrations/m20260328_000002_create_blog_taxonomy_tables.rs",
  [
    ".table(BlogPostTags::Table)",
    "BlogCategories::ParentId",
    ".table(BlogCategoryTranslations::Table)",
    ".to(TaxonomyTerms::Table, TaxonomyTerms::Id)",
  ],
);
requireMarkers("crates/rustok-blog/src/entities/blog_post_tag.rs", [
  'table_name = "blog_post_tags"',
]);

requireMarkers(
  "crates/rustok-forum/src/migrations/m20260328_000001_create_forum_tables.rs",
  [
    ".table(ForumCategories::Table)",
    "ForumCategories::ParentId",
    ".table(ForumCategoryTranslations::Table)",
    "ForumCategoryTranslations::Locale",
  ],
);
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
  "crates/rustok-product/src/migrations/m20260701_000001_create_product_catalog_attributes.rs",
  [
    "CREATE TABLE IF NOT EXISTS catalog_categories",
    "parent_id UUID REFERENCES catalog_categories(id)",
    "CREATE TABLE IF NOT EXISTS catalog_category_closure",
    "CREATE TABLE IF NOT EXISTS product_categories",
  ],
);
requireMarkers("crates/rustok-product/docs/category-locale-contract.md", [
  "Product catalog categories remain a Product-owned tree/closure aggregate.",
  "Product owns category parent/child relations, closure rows, moves, deletion",
  "Taxonomy owns shared vocabulary identities and localized taxonomy route keys",
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
  `Taxonomy ownership boundary checks passed: ${taxonomySourceFiles.length} Taxonomy Rust source files are flat-vocabulary-only; Blog, Forum, Product, and Profiles retain their owner-side attachment/category storage.`,
);
