#!/usr/bin/env node

// Transport <-> schema parity gate for the Blog GraphQL surface.
//
// The Blog schema is owned by `crates/modules/rustok-blog/src/graphql/types.rs` plus the
// root resolvers in `src/graphql/query.rs` and `src/graphql/mutation.rs`. Every Blog client
// package (the two Next.js packages and the two Leptos adapters) sends GraphQL documents
// that are validated by the server *before* execution, so a single stale field name in a
// selection set or in an input object rejects the whole operation at runtime.
//
// This gate derives the Blog object/input field names from the owning Rust source and
// checks every client document against them. It replaces the absent "transport <-> schema
// parity" cross-check that let the Next.js category/tag documents drift away from the
// schema (see `docs/audits/rustok-blog-engineering-audit-2026-10-17.md`, findings B-1..B-4).

import fs from 'node:fs';

const BLOG_CRATE = 'crates/modules/rustok-blog';
const TYPES_PATH = `${BLOG_CRATE}/src/graphql/types.rs`;
const QUERY_PATH = `${BLOG_CRATE}/src/graphql/query.rs`;
const MUTATION_PATH = `${BLOG_CRATE}/src/graphql/mutation.rs`;

const CLIENT_DOCUMENTS = [
  'apps/next-admin/packages/blog/src/api/posts.ts',
  'apps/next-admin/packages/blog/src/api/categories.ts',
  'apps/next-frontend/packages/rustok-blog/src/api/posts.ts',
  `${BLOG_CRATE}/admin/src/transport/graphql_adapter.rs`,
  `${BLOG_CRATE}/admin/src/transport/moderation_adapter.rs`,
  `${BLOG_CRATE}/storefront/src/transport/graphql_adapter.rs`
];

const failures = [];

function fail(message) {
  failures.push(message);
}

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function requireFile(path) {
  if (!fs.existsSync(path)) {
    fail(`missing source artifact ${path}`);
    return '';
  }
  return read(path);
}

// --------------------------------------------------------------------------------------
// Rust source -> GraphQL schema
// --------------------------------------------------------------------------------------

function toCamelCase(name) {
  return name.replace(/_([a-z0-9])/g, (_match, char) => char.toUpperCase());
}

/** Splits `types.rs` into the blocks that carry a `SimpleObject`/`InputObject` derive. */
function parseSchemaTypes(source) {
  const types = new Map();
  const blockPattern =
    /#\[derive\(([^)]*)\)\]\s*(?:#\[graphql\([^\]]*\)\]\s*)*pub struct ([A-Za-z0-9_]+)\s*\{([\s\S]*?)\n\}/g;
  let match;
  while ((match = blockPattern.exec(source)) !== null) {
    const derive = match[1];
    const structName = match[2];
    const body = match[3];
    const isObject = /SimpleObject/.test(derive);
    const isInput = /InputObject/.test(derive);
    if (!isObject && !isInput) {
      continue;
    }
    const graphqlName = graphqlTypeName(source, structName);
    const fields = new Map();
    const fieldPattern =
      /(?:#\[graphql\(skip\)\]\s*)?(?:#\[graphql\([^\]]*\)\]\s*)?pub ([a-z0-9_]+)\s*:\s*([^,\n]+),/g;
    let fieldMatch;
    while ((fieldMatch = fieldPattern.exec(body)) !== null) {
      const rustName = fieldMatch[1];
      const rustType = fieldMatch[2].trim();
      const skipPattern = new RegExp(
        `#\\[graphql\\(skip\\)\\]\\s*(?:#\\[graphql\\([^\\]]*\\)\\]\\s*)*pub ${rustName}\\s*:`
      );
      if (skipPattern.test(body)) {
        continue;
      }
      const renamePattern = new RegExp(
        `#\\[graphql\\(name\\s*=\\s*"([^"]+)"\\)\\]\\s*(?:#\\[graphql\\(skip\\)\\]\\s*)?pub ${rustName}\\s*:`
      );
      const renameMatch = body.match(renamePattern);
      const fieldName = renameMatch ? renameMatch[1] : toCamelCase(rustName);
      fields.set(fieldName, rustType);
    }
    types.set(graphqlName, { structName, fields, kind: isInput ? 'input' : 'object' });
  }
  return types;
}

/** Resolves the GraphQL type name of a Rust struct (`#[graphql(name = "...")]` wins). */
function graphqlTypeName(source, structName) {
  const pattern = new RegExp(
    `#\\[graphql\\(name\\s*=\\s*"([^"]+)"\\)\\]\\s*pub struct ${structName}\\b`
  );
  const match = source.match(pattern);
  return match ? match[1] : structName;
}

/** Extracts the element type of a Rust type expression, or null when it is a scalar. */
function elementType(rustType) {
  const cleaned = rustType.replace(/\s+/g, ' ').trim();
  const vec = cleaned.match(/^Vec<\s*(.+)\s*>$/);
  if (vec) {
    return elementType(vec[1]);
  }
  const option = cleaned.match(/^Option<\s*(.+)\s*>$/);
  if (option) {
    return elementType(option[1]);
  }
  if (/^(String|bool|i32|i64|u32|u64|f32|f64|Uuid|uuid::Uuid|Value|serde_json::Value)$/.test(cleaned)) {
    return null;
  }
  return cleaned;
}

// --------------------------------------------------------------------------------------
// Root resolvers -> root field types
// --------------------------------------------------------------------------------------

function parseRootFields(source, rootType) {
  const fields = new Map();
  const signature = /async fn ([a-z0-9_]+)\s*\(/g;
  let match;
  while ((match = signature.exec(source)) !== null) {
    const fieldName = match[1];
    let index = match.index + match[0].length;
    let depth = 1;
    while (index < source.length && depth > 0) {
      if (source[index] === '(') {
        depth += 1;
      } else if (source[index] === ')') {
        depth -= 1;
      }
      index += 1;
    }
    const tail = source.slice(index, index + 120);
    const returnMatch = tail.match(/^\s*->\s*Result<([A-Za-z0-9_]+(?:<[A-Za-z0-9_]+>)?)>/);
    if (!returnMatch) {
      continue;
    }
    const resolved = elementType(returnMatch[1]);
    if (resolved) {
      fields.set(toCamelCase(fieldName), resolved);
    }
  }
  return { rootType, fields };
}

/**
 * `#[ComplexObject] impl GqlPost { async fn public_comments(...) }` adds resolvers to an
 * existing object type; async-graphql exposes them as camelCase fields of that type.
 */
function parseComplexObjectFields(source, structName) {
  const fields = new Map();
  const implPattern = new RegExp(`impl\\s+${structName}\\s*\\{`, 'g');
  let implMatch;
  while ((implMatch = implPattern.exec(source)) !== null) {
    const start = source.indexOf('{', implMatch.index);
    const end = matchBrace(source, start);
    const body = source.slice(start + 1, end);
    const signature = /async fn ([a-z0-9_]+)\s*\(/g;
    let match;
    while ((match = signature.exec(body)) !== null) {
      const fieldName = match[1];
      let index = match.index + match[0].length;
      let depth = 1;
      while (index < body.length && depth > 0) {
        if (body[index] === '(') {
          depth += 1;
        } else if (body[index] === ')') {
          depth -= 1;
        }
        index += 1;
      }
      const tail = body.slice(index, index + 120);
      const returnMatch = tail.match(/^\s*->\s*Result<([A-Za-z0-9_]+(?:<[A-Za-z0-9_]+>)?)>/);
      if (!returnMatch) {
        continue;
      }
      const resolved = elementType(returnMatch[1]);
      if (resolved) {
        fields.set(toCamelCase(fieldName), resolved);
      }
    }
  }
  return fields;
}

// --------------------------------------------------------------------------------------
// GraphQL document parsing (selection sets only)
// --------------------------------------------------------------------------------------

/** Extracts every GraphQL document literal from a client source file. */
function extractDocuments(path, source) {
  const documents = [];
  const templatePattern = /`([\s\S]*?)`/g;
  const quotedPattern = /"([^"\n]*)"/g;
  const push = (text) => {
    const trimmed = text.trim();
    if (/^(query|mutation)\b/.test(trimmed)) {
      documents.push({ path, text: trimmed });
    }
  };
  let match;
  while ((match = templatePattern.exec(source)) !== null) {
    push(match[1]);
  }
  while ((match = quotedPattern.exec(source)) !== null) {
    push(match[1]);
  }
  return documents;
}

/** Splits a selection set body into `{ name, alias, arguments, hasSelection }` entries. */
function parseSelections(body) {
  const selections = [];
  let index = 0;
  while (index < body.length) {
    const char = body[index];
    if (/\s|,/.test(char)) {
      index += 1;
      continue;
    }
    if (char === '}') {
      break;
    }
    let name = '';
    while (index < body.length && /[A-Za-z0-9_]/.test(body[index])) {
      name += body[index];
      index += 1;
    }
    if (!name) {
      index += 1;
      continue;
    }
    while (index < body.length && /\s/.test(body[index])) {
      index += 1;
    }
    if (body[index] === ':') {
      index += 1;
      while (index < body.length && /\s/.test(body[index])) {
        index += 1;
      }
      let realName = '';
      while (index < body.length && /[A-Za-z0-9_]/.test(body[index])) {
        realName += body[index];
        index += 1;
      }
      name = realName;
      while (index < body.length && /\s/.test(body[index])) {
        index += 1;
      }
    }
    // Arguments.
    if (body[index] === '(') {
      let depth = 0;
      while (index < body.length) {
        if (body[index] === '(') {
          depth += 1;
        } else if (body[index] === ')') {
          depth -= 1;
          if (depth === 0) {
            index += 1;
            break;
          }
        }
        index += 1;
      }
      while (index < body.length && /\s/.test(body[index])) {
        index += 1;
      }
    }
    // Nested selection set.
    let nested = null;
    if (body[index] === '{') {
      const end = matchBrace(body, index);
      nested = body.slice(index + 1, end);
      index = end + 1;
    }
    selections.push({ name, nested });
  }
  return selections;
}

function matchBrace(text, start) {
  let depth = 0;
  for (let index = start; index < text.length; index += 1) {
    if (text[index] === '{') {
      depth += 1;
    } else if (text[index] === '}') {
      depth -= 1;
      if (depth === 0) {
        return index;
      }
    }
  }
  return text.length - 1;
}

/** Returns the body of the first `{ ... }` after the operation header. */
function operationBody(document) {
  const start = document.indexOf('{');
  if (start < 0) {
    return null;
  }
  const end = matchBrace(document, start);
  return document.slice(start + 1, end);
}

// --------------------------------------------------------------------------------------
// Checking
// --------------------------------------------------------------------------------------

function checkSelections(options) {
  const { selections, typeName, context, path, types, rootFields } = options;
  for (const selection of selections) {
    const type = types.get(typeName);
    if (!type) {
      // The selection belongs to a type this gate does not model (a platform type such
      // as `RichTextView`); its fields are owned by another crate.
      continue;
    }
    if (!type.fields.has(selection.name)) {
      fail(
        `${path}: ${context}.${selection.name} is not a field of ${typeName} ` +
          `(known fields: ${[...type.fields.keys()].join(', ')})`
      );
      continue;
    }
    if (!selection.nested) {
      continue;
    }
    const childType = graphqlNameOf(elementType(type.fields.get(selection.name)), types);
    if (!childType || !types.has(childType)) {
      continue;
    }
    checkSelections({
      selections: parseSelections(selection.nested),
      typeName: childType,
      context: `${context}.${selection.name}`,
      path,
      types,
      rootFields
    });
  }
}

/** Maps a Rust type name to the GraphQL type name the schema exposes. */
function graphqlNameOf(rustType, types) {
  if (!rustType) {
    return null;
  }
  if (types.has(rustType)) {
    return rustType;
  }
  for (const [graphqlName, type] of types) {
    if (type.structName === rustType) {
      return graphqlName;
    }
  }
  return null;
}

function checkDocument(document, types, rootFields) {
  const body = operationBody(document.text);
  if (body === null) {
    return;
  }
  const isMutation = /^mutation\b/.test(document.text);
  const root = isMutation ? rootFields.mutation : rootFields.query;
  const selections = parseSelections(body);
  for (const selection of selections) {
    if (!selection.nested) {
      continue;
    }
    const rustType = root.get(selection.name);
    if (!rustType) {
      fail(
        `${document.path}: root field ${selection.name} is not declared by the Blog ` +
          `${isMutation ? 'mutation' : 'query'} root`
      );
      continue;
    }
    const typeName = graphqlNameOf(rustType, types);
    if (!typeName || !types.has(typeName)) {
      continue;
    }
    checkSelections({
      selections: parseSelections(selection.nested),
      typeName,
      context: selection.name,
      path: document.path,
      types,
      rootFields
    });
  }
}

// --------------------------------------------------------------------------------------
// Client input objects
// --------------------------------------------------------------------------------------

/** Extracts the field names of every `export interface X { ... }` in a TypeScript file. */
function parseTsInterfaces(source) {
  const interfaces = new Map();
  const pattern = /export interface ([A-Za-z0-9_]+)[^{]*\{([\s\S]*?)\n\}/g;
  let match;
  while ((match = pattern.exec(source)) !== null) {
    const fields = new Set();
    const fieldPattern = /^\s*([A-Za-z0-9_]+)\??:/gm;
    let fieldMatch;
    while ((fieldMatch = fieldPattern.exec(match[2])) !== null) {
      fields.add(fieldMatch[1]);
    }
    interfaces.set(match[1], fields);
  }
  return interfaces;
}

/** Keys used inside a `filter` object literal or an inline `filter` type literal. */
function collectFilterKeys(source) {
  const keys = new Set();
  const literalPattern = /filter\s*:\s*\{([^}]*)\}/g;
  const typePattern = /filter\??\s*:\s*\{([^}]*)\}/g;
  for (const pattern of [literalPattern, typePattern]) {
    let match;
    while ((match = pattern.exec(source)) !== null) {
      const body = match[1];
      const keyPattern = /([A-Za-z0-9_]+)\s*[?:]/g;
      let keyMatch;
      while ((keyMatch = keyPattern.exec(body)) !== null) {
        keys.add(keyMatch[1]);
      }
    }
  }
  return keys;
}

function checkClientInputs(path, source, types) {
  const interfaces = parseTsInterfaces(source);
  for (const [name, fields] of interfaces) {
    const type = types.get(name);
    if (!type || type.kind !== 'input') {
      continue;
    }
    for (const field of fields) {
      if (!type.fields.has(field)) {
        fail(
          `${path}: the client input interface ${name} declares "${field}", which is not a ` +
            `field of the Blog input type (known fields: ${[...type.fields.keys()].join(', ')})`
        );
      }
    }
  }

  const filterFields = new Set();
  for (const name of ['BlogCategoriesFilter', 'BlogTagsFilter', 'PostsFilter', 'PublicPostsFilter']) {
    const type = types.get(name);
    if (type) {
      for (const field of type.fields.keys()) {
        filterFields.add(field);
      }
    }
  }
  for (const key of collectFilterKeys(source)) {
    if (!filterFields.has(key)) {
      fail(
        `${path}: the client sends the filter key "${key}", which no Blog filter input type ` +
          `declares (known filter fields: ${[...filterFields].sort().join(', ')})`
      );
    }
  }
}

// --------------------------------------------------------------------------------------
// Entry point
// --------------------------------------------------------------------------------------

const typesSource = requireFile(TYPES_PATH);
const types = parseSchemaTypes(typesSource);
for (const [fieldName, rustType] of parseComplexObjectFields(typesSource, 'GqlPost')) {
  const type = types.get('GqlPost');
  if (type && !type.fields.has(fieldName)) {
    type.fields.set(fieldName, rustType);
  }
}
const rootFields = {
  query: parseRootFields(requireFile(QUERY_PATH), 'Query').fields,
  mutation: parseRootFields(requireFile(MUTATION_PATH), 'Mutation').fields
};

if (types.size === 0) {
  fail(`no Blog object/input types were derived from ${TYPES_PATH}`);
}

for (const path of CLIENT_DOCUMENTS) {
  const source = requireFile(path);
  if (!source) {
    continue;
  }
  for (const document of extractDocuments(path, source)) {
    checkDocument(document, types, rootFields);
  }
  if (path.endsWith('.ts')) {
    checkClientInputs(path, source, types);
  }
}

if (failures.length > 0) {
  for (const failure of failures) {
    console.error(`[verify-blog-nextjs-graphql-contract] ${failure}`);
  }
  process.exit(1);
}

console.log(
  `[verify-blog-nextjs-graphql-contract] ${types.size} Blog schema types checked against ` +
    `${CLIENT_DOCUMENTS.length} client documents`
);
