import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, join, relative } from 'node:path';

const REPO_ROOT = resolve('.');

function walkSync(dir, fileList = []) {
  const files = readdirSync(dir);
  for (const file of files) {
    const fullPath = join(dir, file);
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      if (
        file === 'target' ||
        file === 'node_modules' ||
        file === '.git' ||
        file === 'tests' ||
        file === 'benches' ||
        file === 'examples' ||
        file === '__tests__'
      ) {
        continue;
      }
      walkSync(fullPath, fileList);
    } else if (file.endsWith('.rs')) {
      fileList.push(fullPath);
    }
  }
  return fileList;
}

let errors = [];

// 1. Verify No Ad-Hoc Prints in Core Runtime Crates
console.log('Checking for ad-hoc prints (println!, eprintln!, dbg!) in production code...');
const targetDirs = [
  join(REPO_ROOT, 'crates', 'modules'),
  join(REPO_ROOT, 'crates', 'libs'),
  join(REPO_ROOT, 'crates', 'workers'),
  join(REPO_ROOT, 'apps', 'server', 'src'),
];

for (const dir of targetDirs) {
  const files = walkSync(dir);
  for (const filePath of files) {
    const rel = relative(REPO_ROOT, filePath).replace(/\\/g, '/');
    if (
      rel.includes('build.rs') ||
      rel.includes('/tests/') ||
      rel.includes('_test.rs') ||
      rel.includes('/examples/') ||
      rel.includes('/src/bin/') ||
      rel.includes('/cli/') ||
      rel.includes('crates/utils/rustok-doctor') ||
      rel.includes('tests.rs')
    ) {
      continue;
    }

    const content = readFileSync(filePath, 'utf-8');
    const lines = content.split('\n');
    let inTestMod = false;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (line.includes('#[cfg(test)]') || line.startsWith('mod tests {') || line.startsWith('mod tests;')) {
        inTestMod = true;
      }
      if (inTestMod) {
        continue;
      }
      if (line.startsWith('//') || line.startsWith('/*') || line.startsWith('*')) {
        continue;
      }
      // Check for forbidden macros
      if (/\b(println!|eprintln!|dbg!)/.test(line)) {
        // Exception for multi-process test harness in comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs
        if (rel.includes('comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs') && line.includes('CHILD_READY_MARKER')) {
          continue;
        }
        errors.push(`Ad-hoc print found in ${rel}:${i + 1}: ${line}`);
      }
    }
  }
}

// 2. Verify Span Field Hygiene (PII & User-Authored Payload Redaction)
console.log('Checking for span field hygiene (PII and user payload redaction)...');
const spanScanDirs = [
  join(REPO_ROOT, 'crates', 'modules'),
  join(REPO_ROOT, 'apps', 'server', 'src'),
];

const FORBIDDEN_SPAN_PATTERNS = [
  /fields\s*\([^)]*email\s*=\s*%/,
  /fields\s*\([^)]*password\s*=\s*%/,
  /fields\s*\([^)]*token\s*=\s*%input/,
  /fields\s*\([^)]*title\s*=\s*%input/,
  /fields\s*\([^)]*name\s*=\s*%input\.name/,
];

for (const dir of spanScanDirs) {
  const files = walkSync(dir);
  for (const filePath of files) {
    const rel = relative(REPO_ROOT, filePath).replace(/\\/g, '/');
    const content = readFileSync(filePath, 'utf-8');
    for (const pattern of FORBIDDEN_SPAN_PATTERNS) {
      if (pattern.test(content)) {
        errors.push(`Forbidden span field logging pattern ${pattern} found in ${rel}`);
      }
    }
  }
}

// 3. Verify Telemetry Lifecycle Safety
console.log('Checking Telemetry Lifecycle Safety...');
const telemetryLibPath = join(REPO_ROOT, 'crates', 'libs', 'rustok-telemetry', 'src', 'lib.rs');
const telemetryOtelPath = join(REPO_ROOT, 'crates', 'libs', 'rustok-telemetry', 'src', 'otel.rs');
const telemetryLib = readFileSync(telemetryLibPath, 'utf-8');
const telemetryOtel = readFileSync(telemetryOtelPath, 'utf-8');

if (!telemetryOtel.includes('pub fn build_otel_layer')) {
  errors.push('rustok-telemetry must provide synchronous `build_otel_layer` for startup safety.');
}
if (telemetryLib.includes('tokio::task::block_in_place') || telemetryLib.includes('Handle::current()')) {
  errors.push('rustok-telemetry::init must not call `block_in_place` or `Handle::current()` outside active Tokio context.');
}

// 4. Verify Health & Readiness Probe Architecture
console.log('Checking Health & Readiness Probe Architecture...');
const healthControllerPath = join(REPO_ROOT, 'apps', 'server', 'src', 'controllers', 'health.rs');
const healthController = readFileSync(healthControllerPath, 'utf-8');

const requiredEndpoints = [
  '/health',
  '/health/live',
  '/health/ready',
  '/health/runtime',
  '/health/modules',
];
for (const endpoint of requiredEndpoints) {
  if (!healthController.includes(`"${endpoint}"`)) {
    errors.push(`Health controller missing required route: ${endpoint}`);
  }
}

if (!healthController.includes('READINESS_TIMEOUT')) {
  errors.push('Health controller must enforce READINESS_TIMEOUT.');
}
if (!healthController.includes('circuit_open_for')) {
  errors.push('Health controller must implement circuit breaker protection for guarded checks.');
}
if (!healthController.includes('buffer_unordered(READINESS_CONCURRENCY)')) {
  errors.push('Health controller must bound concurrency using buffer_unordered(READINESS_CONCURRENCY).');
}

if (errors.length > 0) {
  console.error('\n❌ Observability, Telemetry & Health Probe Verification FAILED:');
  for (const err of errors) {
    console.error(`  - ${err}`);
  }
  process.exit(1);
}

console.log('\n✅ All Observability, Distributed Tracing & Telemetry (CC-10) invariants verified successfully.');
