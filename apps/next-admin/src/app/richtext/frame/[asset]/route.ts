import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const ASSET_TYPES: Record<string, string> = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8'
};

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ asset: string }> }
) {
  const { asset } = await params;
  if (!/^[a-z0-9._-]+\.(?:css|html|js)$/.test(asset)) {
    return new Response('Not found', { status: 404 });
  }
  const root = await richTextDistRoot();
  const manifest = JSON.parse(await readFile(resolve(root, 'asset-manifest.json'), 'utf8')) as Record<string, string>;
  if (!Object.values(manifest).includes(asset)) return new Response('Not found', { status: 404 });
  const body = await readFile(resolve(root, asset));
  const contentType = ASSET_TYPES[asset.slice(asset.lastIndexOf('.'))] ?? 'application/octet-stream';
  return new Response(body, {
    headers: {
      'cache-control': 'public, max-age=31536000, immutable',
      'content-security-policy': "default-src 'none'; frame-ancestors 'self'; object-src 'none'; base-uri 'none'; form-action 'none'",
      'content-type': contentType,
      'permissions-policy': 'camera=(), microphone=(), geolocation=(), payment=()',
      'referrer-policy': 'no-referrer',
      'x-content-type-options': 'nosniff',
      'x-frame-options': 'SAMEORIGIN'
    }
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
