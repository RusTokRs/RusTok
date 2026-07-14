import { readFile } from 'node:fs/promises';
import process from 'node:process';

const root = process.cwd();
const [lib, services, graphql] = await Promise.all([
  readFile(`${root}/crates/rustok-pages/src/lib.rs`, 'utf8'),
  readFile(`${root}/crates/rustok-pages/src/services/mod.rs`, 'utf8'),
  readFile(`${root}/crates/rustok-pages/src/graphql/scenario_baseline.rs`, 'utf8'),
]);

const failures = [];
if (!services.includes('PageBuilderScenarioBaselineRecord')) {
  failures.push('services module does not export PageBuilderScenarioBaselineRecord');
}
if (!lib.includes('PageBuilderScenarioBaselineRecord')) {
  failures.push('rustok-pages crate root does not export PageBuilderScenarioBaselineRecord');
}
if (!graphql.includes('PageBuilderScenarioBaselineRecord')) {
  failures.push('scenario baseline GraphQL API does not use the typed promotion record');
}

if (failures.length > 0) {
  console.error('Pages scenario baseline root export verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Pages scenario baseline root exports verified.');
