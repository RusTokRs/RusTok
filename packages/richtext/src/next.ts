import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const FRAME_SECURITY_HEADERS = {
  'cache-control': 'no-store',
  'content-security-policy':
    "default-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'",
  'permissions-policy': 'camera=(), microphone=(), geolocation=(), payment=()',
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  'x-frame-options': 'SAMEORIGIN'
};

const ASSET_SECURITY_HEADERS = {
  'cache-control': 'public, max-age=31536000, immutable',
  'content-security-policy':
    "default-src 'none'; frame-ancestors 'self'; object-src 'none'; base-uri 'none'; form-action 'none'",
  'permissions-policy': 'camera=(), microphone=(), geolocation=(), payment=()',
  'referrer-policy': 'no-referrer',
  'x-content-type-options': 'nosniff',
  'x-frame-options': 'SAMEORIGIN'
};

const ASSET_TYPES: Record<string, string> = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8'
};

export async function richTextFrameResponse(): Promise<Response> {
  const root = await richTextDistRoot();
  const manifest = await richTextManifest(root);
  const html = await readFile(resolve(root, manifest.frame));
  return new Response(html, {
    headers: { ...FRAME_SECURITY_HEADERS, 'content-type': 'text/html; charset=utf-8' }
  });
}

export async function richTextFrameAssetResponse(asset: string): Promise<Response> {
  if (!/^[a-z0-9._-]+\.(?:css|html|js)$/.test(asset)) {
    return new Response('Not found', { status: 404 });
  }
  const root = await richTextDistRoot();
  const manifest = await richTextManifest(root);
  if (!Object.values(manifest).includes(asset)) {
    return new Response('Not found', { status: 404 });
  }
  const body = await readFile(resolve(root, asset));
  const extension = asset.slice(asset.lastIndexOf('.'));
  return new Response(body, {
    headers: {
      ...ASSET_SECURITY_HEADERS,
      'content-type': ASSET_TYPES[extension] ?? 'application/octet-stream'
    }
  });
}

async function richTextManifest(root: string): Promise<Record<string, string> & { frame: string }> {
  return JSON.parse(
    await readFile(resolve(root, 'asset-manifest.json'), 'utf8')
  ) as Record<string, string> & { frame: string };
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
      // Try the next supported workspace or deployment layout.
    }
  }
  throw new Error('The @rustok/richtext immutable assets are not built');
}

