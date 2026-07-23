import { createHash } from 'node:crypto';
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dist = resolve(root, 'dist');
const temporary = resolve(root, '.build');
await rm(dist, { recursive: true, force: true });
await rm(temporary, { recursive: true, force: true });
await mkdir(dist, { recursive: true });
await mkdir(temporary, { recursive: true });

const temporaryBundle = resolve(temporary, 'frame.js');
await build({
  entryPoints: [resolve(root, 'src/frame/runtime.ts')],
  outfile: temporaryBundle,
  bundle: true,
  format: 'iife',
  platform: 'browser',
  target: ['es2022'],
  minify: true,
  legalComments: 'none',
  sourcemap: false
});

await build({
  entryPoints: [resolve(root, 'src/index.ts')],
  outfile: resolve(temporary, 'core.mjs'),
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: ['es2022'],
  minify: false,
  legalComments: 'none',
  sourcemap: false
});

const bundle = await readFile(temporaryBundle);
const css = await readFile(resolve(root, 'src/frame/frame.css'));
const releaseHash = createHash('sha256').update(bundle).update(css).digest('hex').slice(0, 16);
const scriptName = `richtext-frame.${releaseHash}.js`;
const styleName = `richtext-frame.${releaseHash}.css`;
await writeFile(resolve(dist, scriptName), bundle);
await writeFile(resolve(dist, styleName), css);
await cp(resolve(temporary, 'core.mjs'), resolve(dist, 'core.mjs'));

const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta name="referrer" content="no-referrer">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'">
  <link rel="stylesheet" href="/richtext/frame/${styleName}">
  <title>Richtext editor</title>
</head>
<body>
  <main class="richtext-frame">
    <div id="richtext-toolbar" class="richtext-toolbar" role="toolbar"></div>
    <div id="richtext-editor"></div>
  </main>
  <script src="/richtext/frame/${scriptName}" defer></script>
</body>
</html>
`;
const htmlHash = createHash('sha256').update(html).digest('hex').slice(0, 16);
const frameName = `frame.${htmlHash}.html`;
await writeFile(resolve(dist, frameName), html);
await writeFile(
  resolve(dist, 'asset-manifest.json'),
  `${JSON.stringify({ revision: 1, frame: frameName, script: scriptName, style: styleName }, null, 2)}\n`
);
await cp(resolve(root, 'README.md'), resolve(dist, 'README.md'));
await rm(temporary, { recursive: true, force: true });
