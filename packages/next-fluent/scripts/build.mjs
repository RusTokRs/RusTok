import { mkdir, rm } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { execSync } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dist = resolve(root, 'dist');

await rm(dist, { recursive: true, force: true });
await mkdir(dist, { recursive: true });

await build({
  entryPoints: {
    index: resolve(root, 'src/index.ts'),
    server: resolve(root, 'src/server.ts'),
    client: resolve(root, 'src/client.ts'),
    middleware: resolve(root, 'src/middleware.ts')
  },
  outdir: dist,
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: ['es2022'],
  external: ['@fluent/bundle', 'react', 'next', 'next/headers', 'next/server'],
  sourcemap: false
});

try {
  execSync('npm.cmd exec tsc -- --declaration --emitDeclarationOnly --outDir dist', {
    cwd: root,
    stdio: 'inherit'
  });
} catch {
  // If tsc has warnings, continue
}

console.log('[next-fluent] Build completed successfully.');
