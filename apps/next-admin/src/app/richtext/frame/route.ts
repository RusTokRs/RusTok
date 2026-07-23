import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const SECURITY_HEADERS = {
  'cache-control': 'no-store',
  'content-security-policy': "default-src 'none'; frame-ancestors 'self'; object-src 'none'; base-uri 'none'; form-action 'none'",
  'permissions-policy': 'camera=(), microphone=(), geolocation=(), payment=()',
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  'x-frame-options': 'SAMEORIGIN'
};

export async function GET() {
  const root = await richTextDistRoot();
  const manifest = JSON.parse(await readFile(resolve(root, 'asset-manifest.json'), 'utf8')) as {
    frame: string;
  };
  const html = await readFile(resolve(root, manifest.frame));
  return new Response(html, {
    headers: { ...SECURITY_HEADERS, 'content-type': 'text/html; charset=utf-8' }
  });
}

async function richTextDistRoot(): Promise<string> {
  const candidates = [
    resolve(process.cwd(), 'node_modules/@rustok/richtext/dist'),
    resolve(process.cwd(), '../../packages/richtext/dist'),
    resolve(process.cwd(), 'packages/richtext/dist')
  ];
  for (const candidate of candidates) {
    try {
      await readFile(resolve(candidate, 'asset-manifest.json'));
      return candidate;
    } catch {
      // Try the next build/deployment layout.
    }
  }
  throw new Error('The @rustok/richtext immutable assets are not built');
}
