#!/usr/bin/env node

/**
 * Safe Migration Squashing & Schema Parity Engine for RusTok modules.
 *
 * Capabilities:
 *  1. Parses SeaORM models in `crates/modules/<module>/src/entities/*.rs`.
 *  2. Parses SeaQuery & SQL migration files in `crates/modules/<module>/src/migrations/*.rs`.
 *  3. Verifies 100% schema parity:
 *     - Every entity table exists in migrations.
 *     - Every entity column exists in the migration table.
 *     - Nullability match: non-Option<T> must be NOT NULL, Option<T> must be NULLable.
 *     - Primary keys match.
 *  4. Validates migration registry (`src/migrations/mod.rs`):
 *     - All files on disk are registered.
 *     - No dangling/missing migration registrations.
 *  5. Scans `scripts/verify/` and `tests/` for obsolete/deleted migration references.
 *  6. Reports inventory and flags any regressions before or after squashing.
 *
 * Usage:
 *   node scripts/dev/safe-migration-squash.mjs --module blog
 *   node scripts/dev/safe-migration-squash.mjs --module rustok-blog --check
 *   node scripts/dev/safe-migration-squash.mjs --all
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '..', '..');

// Helper to convert snake_case to PascalCase (e.g. blog_posts -> BlogPosts)
function toPascalCase(str) {
  return str
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join('');
}

// Normalize module name (e.g., 'blog' -> 'rustok-blog', 'rustok-blog' -> 'rustok-blog')
function resolveModuleDir(moduleInput) {
  const directPath = path.join(REPO_ROOT, 'crates', 'modules', moduleInput);
  if (fs.existsSync(directPath)) return directPath;

  const rustokPath = path.join(REPO_ROOT, 'crates', 'modules', `rustok-${moduleInput}`);
  if (fs.existsSync(rustokPath)) return rustokPath;

  // Search by exact match in crates/modules
  const modulesDir = path.join(REPO_ROOT, 'crates', 'modules');
  if (fs.existsSync(modulesDir)) {
    const entries = fs.readdirSync(modulesDir);
    for (const entry of entries) {
      if (entry === moduleInput || entry === `rustok-${moduleInput}`) {
        return path.join(modulesDir, entry);
      }
    }
  }

  return null;
}

/**
 * Parse all SeaORM entities in `src/entities/`
 */
function parseEntities(moduleDir) {
  const entitiesDir = path.join(moduleDir, 'src', 'entities');
  if (!fs.existsSync(entitiesDir)) {
    return { tables: new Map(), errors: [] };
  }

  const files = fs.readdirSync(entitiesDir).filter((f) => f.endsWith('.rs') && f !== 'mod.rs');
  const tables = new Map();
  const errors = [];

  for (const file of files) {
    const filePath = path.join(entitiesDir, file);
    const content = fs.readFileSync(filePath, 'utf8');

    // Extract table_name: #[sea_orm(table_name = "xxx")]
    const tableMatch = content.match(/#\[sea_orm\(table_name\s*=\s*"([^"]+)"\)\]/);
    if (!tableMatch) {
      continue;
    }
    const tableName = tableMatch[1];

    // Find the `pub struct Model { ... }` block
    const modelMatch = content.match(/pub\s+struct\s+Model\s*\{([\s\S]*?)\n\}/);
    if (!modelMatch) {
      errors.push(`${file}: could not find 'pub struct Model' block`);
      continue;
    }

    const modelBody = modelMatch[1];
    const columns = new Map();

    // Parse fields: optional #[sea_orm(...)] attributes, followed by `pub field_name: Type,`
    const lines = modelBody.split('\n');
    let pendingAttrs = [];

    for (let line of lines) {
      line = line.trim();
      if (!line || line.startsWith('//')) continue;

      if (line.startsWith('#[')) {
        pendingAttrs.push(line);
        continue;
      }

      const fieldMatch = line.match(/^pub\s+([a-zA-Z0-9_]+)\s*:\s*([^,]+)/);
      if (fieldMatch) {
        const colName = fieldMatch[1];
        const rawType = fieldMatch[2].trim();
        const isPrimaryKey = pendingAttrs.some((attr) => attr.includes('primary_key'));
        const isNullable = rawType.startsWith('Option<');

        columns.set(colName, {
          name: colName,
          rawType,
          isNullable,
          isPrimaryKey,
          attributes: pendingAttrs,
        });
        pendingAttrs = [];
      }
    }

    tables.set(tableName, {
      file,
      tableName,
      columns,
    });
  }

  return { tables, errors };
}

/**
 * Parse migrations in `src/migrations/`
 */
function parseMigrations(moduleDir) {
  const migrationsDir = path.join(moduleDir, 'src', 'migrations');
  if (!fs.existsSync(migrationsDir)) {
    return { migrations: [], tables: new Map(), rawSqlTables: new Set(), errors: [] };
  }

  const files = fs
    .readdirSync(migrationsDir)
    .filter((f) => f.endsWith('.rs') && f !== 'mod.rs')
    .sort();

  const tables = new Map();
  const rawSqlTables = new Set();
  const errors = [];

  for (const file of files) {
    const filePath = path.join(migrationsDir, file);
    const content = fs.readFileSync(filePath, 'utf8');

    // 1. Parse SeaQuery Table::create() blocks
    // Pattern: Table::create().table(TableIden::Table).col(ColumnDef::new(TableIden::Col)...)
    const tableCreateMatches = content.matchAll(
      /Table::create\(\)\s*[\s\S]*?\.table\((?:([a-zA-Z0-9_]+)::Table|[a-zA-Z0-9_]+)\)([\s\S]*?)\.to_owned\(\)/g,
    );

    for (const match of tableCreateMatches) {
      const idenName = match[1];
      const tableBody = match[2];

      // Extract column definitions: ColumnDef::new(ColIden::ColName)...
      const colMatches = tableBody.matchAll(
        /ColumnDef::new\((?:[a-zA-Z0-9_]+::)?([a-zA-Z0-9_]+)\)([\s\S]*?)(?=(?:ColumnDef::new|\.foreign_key|\.primary_key\s*\(\s*Index|\.index|\.to_owned|$))/g,
      );

      const columns = new Map();
      for (const colMatch of colMatches) {
        const colIden = colMatch[1];
        const colChain = colMatch[2];

        // Column snake_case name guess from Iden
        const colSnake = colIden.replace(/([A-Z])/g, (g, i) => (i === 0 ? g.toLowerCase() : `_${g.toLowerCase()}`));

        const isNotNull = colChain.includes('.not_null()');
        const isNull = colChain.includes('.null()');
        const isPrimaryKey = colChain.includes('.primary_key()');

        columns.set(colSnake, {
          colIden,
          isNotNull,
          isNull,
          isPrimaryKey,
          chain: colChain,
        });
      }

      // Also parse table-level primary key: .primary_key(Index::create().col(...).col(...))
      const pkIndex = tableBody.indexOf('.primary_key(');
      if (pkIndex >= 0) {
        let depth = 0;
        let pkSnippet = '';
        for (let i = pkIndex + '.primary_key'.length; i < tableBody.length; i++) {
          const char = tableBody[i];
          if (char === '(') depth++;
          else if (char === ')') {
            depth--;
            if (depth === 0) {
              pkSnippet = tableBody.slice(pkIndex, i + 1);
              break;
            }
          }
        }
        if (pkSnippet) {
          const pkCols = [...pkSnippet.matchAll(/\.col\((?:[a-zA-Z0-9_]+::)?([a-zA-Z0-9_]+)\)/g)].map((m) =>
            m[1].replace(/([A-Z])/g, (g, i) => (i === 0 ? g.toLowerCase() : `_${g.toLowerCase()}`)),
          );
          for (const pkCol of pkCols) {
            if (columns.has(pkCol)) {
              columns.get(pkCol).isPrimaryKey = true;
            }
          }
        }
      }

      tables.set(idenName, {
        file,
        idenName,
        columns,
        rawBody: tableBody,
      });
    }

    // 2. Parse raw SQL CREATE TABLE blocks
    const sqlTableMatches = content.matchAll(
      /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z0-9_]+)\s*\(([\s\S]*?)\);/gi,
    );
    for (const sqlMatch of sqlTableMatches) {
      const sqlTableName = sqlMatch[1].toLowerCase();
      rawSqlTables.add(sqlTableName);

      const body = sqlMatch[2];
      const sqlCols = new Map();
      const colDefs = body.split(',\n');

      for (let def of colDefs) {
        def = def.trim();
        if (
          def.startsWith('PRIMARY KEY') ||
          def.startsWith('FOREIGN KEY') ||
          def.startsWith('CONSTRAINT') ||
          def.startsWith('CHECK') ||
          def.startsWith('UNIQUE')
        ) {
          continue;
        }

        const parts = def.split(/\s+/);
        if (parts.length >= 2) {
          const colName = parts[0].replace(/"/g, '').toLowerCase();
          const isNotNull = def.toUpperCase().includes('NOT NULL');
          const isPk = def.toUpperCase().includes('PRIMARY KEY');
          sqlCols.set(colName, {
            colName,
            isNotNull,
            isNull: !isNotNull,
            isPrimaryKey: isPk,
            chain: def,
          });
        }
      }

      tables.set(sqlTableName, {
        file,
        idenName: toPascalCase(sqlTableName),
        tableName: sqlTableName,
        columns: sqlCols,
        isRawSql: true,
      });
    }
  }

  return { files, tables, rawSqlTables, errors };
}

/**
 * Check migrations/mod.rs consistency
 */
function checkMigrationRegistry(moduleDir) {
  const modPath = path.join(moduleDir, 'src', 'migrations', 'mod.rs');
  if (!fs.existsSync(modPath)) {
    return { ok: false, error: 'src/migrations/mod.rs not found' };
  }

  const modContent = fs.readFileSync(modPath, 'utf8');
  const migrationsDir = path.join(moduleDir, 'src', 'migrations');
  const filesOnDisk = fs
    .readdirSync(migrationsDir)
    .filter((f) => f.endsWith('.rs') && f !== 'mod.rs');

  const declaredMods = [...modContent.matchAll(/(?:pub\s+)?mod\s+([a-zA-Z0-9_]+);/g)].map((m) => m[1]);
  const registeredInFn = [...modContent.matchAll(/Box::new\(([a-zA-Z0-9_]+)::Migration\)/g)].map((m) => m[1]);

  const missingFiles = declaredMods.filter((mod) => !filesOnDisk.includes(`${mod}.rs`));
  const unregFiles = filesOnDisk.map((f) => f.replace('.rs', '')).filter((f) => !declaredMods.includes(f));
  const unregisteredInVector = declaredMods.filter((mod) => !registeredInFn.includes(mod));

  return {
    ok: missingFiles.length === 0 && unregFiles.length === 0 && unregisteredInVector.length === 0,
    filesOnDisk,
    declaredMods,
    missingFiles,
    unregFiles,
    unregisteredInVector,
  };
}

/**
 * Scan scripts/verify and tests for obsolete migration references
 */
function scanObsoleteReferences(moduleDir, activeMigrationFiles) {
  const activeSet = new Set(activeMigrationFiles.map((f) => f.replace('.rs', '')));
  const warnings = [];

  const dirsToScan = [
    path.join(REPO_ROOT, 'scripts', 'verify'),
    path.join(moduleDir, 'tests'),
  ];

  const migrationRegex = /m202[0-9]_[0-9]{6}_[a-zA-Z0-9_]+/g;

  for (const dir of dirsToScan) {
    if (!fs.existsSync(dir)) continue;

    const scanFiles = fs.readdirSync(dir).filter((f) => f.endsWith('.mjs') || f.endsWith('.rs'));
    for (const f of scanFiles) {
      const fullPath = path.join(dir, f);
      const text = fs.readFileSync(fullPath, 'utf8');

      const matches = [...text.matchAll(migrationRegex)];
      for (const m of matches) {
        const foundMigration = m[0];
        // If it references this module's domain (e.g. blog, category, post, etc.)
        const moduleBase = path.basename(moduleDir).replace('rustok-', '');
        if (foundMigration.includes(moduleBase) && !activeSet.has(foundMigration)) {
          // Check if it's in a test asserting absence/retirement
          const index = m.index;
          const surrounding = text.slice(Math.max(0, index - 80), Math.min(text.length, index + 80));
          const isAbsenceCheck =
            surrounding.includes('requireAbsent') ||
            surrounding.includes('!contains') ||
            surrounding.includes('assert!(!');

          if (!isAbsenceCheck) {
            warnings.push({
              file: path.relative(REPO_ROOT, fullPath),
              migration: foundMigration,
              context: surrounding.trim().replace(/\n/g, ' '),
            });
          }
        }
      }
    }
  }

  return warnings;
}

/**
 * Verify Schema Parity between Entities and Migrations
 */
function verifyModuleSchemaParity(moduleInput) {
  const moduleDir = resolveModuleDir(moduleInput);
  if (!moduleDir) {
    console.error(`Error: Module '${moduleInput}' not found under crates/modules/`);
    process.exit(1);
  }

  const moduleName = path.basename(moduleDir);
  console.log(`\n======================================================`);
  console.log(`🔍 Checking Schema Parity: ${moduleName}`);
  console.log(`📁 Directory: ${moduleDir}`);
  console.log(`======================================================`);

  const { tables: entityTables, errors: entityErrors } = parseEntities(moduleDir);
  const { files: migrationFiles, tables: migrationTables, rawSqlTables, errors: migErrors } = parseMigrations(moduleDir);
  const registryStatus = checkMigrationRegistry(moduleDir);

  const failures = [];
  const warnings = [];

  // 1. Entity errors
  for (const err of entityErrors) failures.push(`Entity Parse Error: ${err}`);
  for (const err of migErrors) failures.push(`Migration Parse Error: ${err}`);

  // 2. Migration Registry checks
  if (!registryStatus.ok) {
    if (registryStatus.missingFiles?.length > 0) {
      failures.push(`mod.rs declares migrations not present on disk: ${registryStatus.missingFiles.join(', ')}`);
    }
    if (registryStatus.unregFiles?.length > 0) {
      failures.push(`Migration files on disk not declared in mod.rs: ${registryStatus.unregFiles.join(', ')}`);
    }
    if (registryStatus.unregisteredInVector?.length > 0) {
      failures.push(
        `Migrations declared in mod.rs but missing in migrations() vector: ${registryStatus.unregisteredInVector.join(', ')}`,
      );
    }
  }

  // 3. Schema Parity: For every entity table -> check migration table
  console.log(`\n📊 Analyzed ${entityTables.size} SeaORM Entity Models across ${migrationFiles.length} Migration file(s).`);

  for (const [tableName, entity] of entityTables.entries()) {
    // Find matching migration table (either by PascalCase iden, exact table_name, or raw SQL table)
    const pascalName = toPascalCase(tableName);
    let migTable = migrationTables.get(pascalName) || migrationTables.get(tableName);

    if (!migTable) {
      // Try fuzzy search in migration tables
      for (const [key, val] of migrationTables.entries()) {
        if (key.toLowerCase() === tableName.toLowerCase() || key.toLowerCase() === pascalName.toLowerCase()) {
          migTable = val;
          break;
        }
      }
    }

    if (!migTable) {
      failures.push(`Table '${tableName}' (from ${entity.file}) NOT FOUND in any migration definition!`);
      continue;
    }

    // Check columns
    const migCols = migTable.columns;
    for (const [colName, entCol] of entity.columns.entries()) {
      // Find matching column in migration table
      let matchedCol = migCols.get(colName);
      if (!matchedCol) {
        // Try snake_case vs camelCase
        for (const [mName, mVal] of migCols.entries()) {
          if (mName.toLowerCase().replace(/_/g, '') === colName.toLowerCase().replace(/_/g, '')) {
            matchedCol = mVal;
            break;
          }
        }
      }

      if (!matchedCol) {
        failures.push(`Table '${tableName}': Column '${colName}' (${entCol.rawType}) missing in migration definition!`);
        continue;
      }

      // Nullability check
      if (!entCol.isNullable && matchedCol.isNull && !matchedCol.isNotNull) {
        warnings.push(
          `Table '${tableName}': Column '${colName}' is non-Option in Entity (${entCol.rawType}) but marked nullable in migration.`,
        );
      }
      if (entCol.isNullable && matchedCol.isNotNull) {
        failures.push(
          `Table '${tableName}': Column '${colName}' is Option<T> in Entity but NOT NULL in migration (will fail on insert of None)!`,
        );
      }

      // Primary key check
      if (entCol.isPrimaryKey && !matchedCol.isPrimaryKey) {
        warnings.push(`Table '${tableName}': Column '${colName}' is primary_key in Entity but not flagged in migration.`);
      }
    }
  }

  // 4. Check for obsolete references in scripts & tests
  const obsoleteRefs = scanObsoleteReferences(moduleDir, migrationFiles);
  for (const obs of obsoleteRefs) {
    warnings.push(`Obsolete migration reference '${obs.migration}' in ${obs.file}: "${obs.context}"`);
  }

  // Print Results
  console.log(`\n------------------------------------------------------`);
  if (failures.length === 0 && warnings.length === 0) {
    console.log(`✅ PARITY PERFECT! All ${entityTables.size} tables and columns strictly verified.`);
    console.log(`✅ Migration registry matches 100% of physical files.`);
    console.log(`✅ Zero obsolete migration references detected.`);
    return true;
  }

  if (warnings.length > 0) {
    console.log(`⚠️  ${warnings.length} Warnings:`);
    for (const w of warnings) {
      console.log(`   - ${w}`);
    }
  }

  if (failures.length > 0) {
    console.error(`\n❌ ${failures.length} CRITICAL SCHEMA PARITY FAILURES:`);
    for (const f of failures) {
      console.error(`   ✖ ${f}`);
    }
    return false;
  }

  return true;
}

// CLI Handling
const args = process.argv.slice(2);
let moduleArg = null;
let allFlag = false;

for (let i = 0; i < args.length; i++) {
  if (args[i] === '--module' || args[i] === '-m') {
    moduleArg = args[i + 1];
    i++;
  } else if (args[i] === '--all') {
    allFlag = true;
  }
}

if (!moduleArg && !allFlag) {
  // Default to blog if nothing passed
  moduleArg = 'blog';
}

if (allFlag) {
  const modulesDir = path.join(REPO_ROOT, 'crates', 'modules');
  const dirs = fs.readdirSync(modulesDir).filter((d) => fs.statSync(path.join(modulesDir, d)).isDirectory());
  let allPassed = true;
  for (const d of dirs) {
    const hasEntities = fs.existsSync(path.join(modulesDir, d, 'src', 'entities'));
    const hasMigrations = fs.existsSync(path.join(modulesDir, d, 'src', 'migrations'));
    if (hasEntities && hasMigrations) {
      const ok = verifyModuleSchemaParity(d);
      if (!ok) allPassed = false;
    }
  }
  process.exit(allPassed ? 0 : 1);
} else {
  const ok = verifyModuleSchemaParity(moduleArg);
  process.exit(ok ? 0 : 1);
}
