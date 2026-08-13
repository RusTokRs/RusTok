import { readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const contractPath = path.join(
  repoRoot,
  'crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json'
);

type BrowserContract = {
  output: {
    environment: string;
    default_path: string;
  };
};

function fail(message: string): never {
  throw new Error(`Pages published metadata browser setup failed: ${message}`);
}

function resolveOutput(contract: BrowserContract): string {
  const raw = process.env[contract.output.environment];
  if (
    raw !== undefined &&
    (raw.length === 0 || raw.length > 16_384 || /[\u0000]/u.test(raw))
  ) {
    fail(`${contract.output.environment} is outside the bounded output input`);
  }
  const requested = raw || contract.output.default_path;
  const absolute = path.isAbsolute(requested)
    ? path.resolve(requested)
    : path.resolve(repoRoot, requested);
  const targetRoot = path.resolve(repoRoot, 'target');
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    fail('browser evidence output must remain inside repository target/');
  }
  return absolute;
}

export default function globalSetup(): void {
  const contract = JSON.parse(
    readFileSync(contractPath, 'utf8')
  ) as BrowserContract;
  rmSync(resolveOutput(contract), { force: true });
}
