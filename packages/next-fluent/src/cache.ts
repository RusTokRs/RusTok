import { FluentBundle, FluentResource, type FluentFunction } from '@fluent/bundle';
import { createDefaultFunctions } from './functions';

export interface CreateFluentBundleOptions {
  useIsolating?: boolean;
  functions?: Record<string, FluentFunction>;
  disableCache?: boolean;
}

const MAX_RESOURCE_CACHE = 1000;
const MAX_BUNDLE_CACHE = 500;

// FNV-1a 64-bit hash for fast, deterministic string hashing without external dependencies
export function fnv1a64(input: string): string {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;

  for (let i = 0; i < input.length; i++) {
    hash ^= BigInt(input.charCodeAt(i));
    hash = (hash * prime) & mask;
  }

  return hash.toString(16).padStart(16, '0');
}

export function computeSourceHash(source: string | readonly string[]): string {
  if (typeof source === 'string') {
    return `${source.length}:${fnv1a64(source)}`;
  }
  const joined = source.join('\u0000');
  return `${source.length}:${joined.length}:${fnv1a64(joined)}`;
}

const resourceCache = new Map<string, FluentResource>();
const bundleCache = new Map<string, FluentBundle>();

export function getOrCreateResource(source: string): FluentResource {
  const hash = `${source.length}:${fnv1a64(source)}`;
  let res = resourceCache.get(hash);
  if (!res) {
    if (resourceCache.size >= MAX_RESOURCE_CACHE) {
      // Evict oldest 20%
      const keys = Array.from(resourceCache.keys()).slice(0, Math.floor(MAX_RESOURCE_CACHE * 0.2));
      for (const k of keys) resourceCache.delete(k);
    }
    res = new FluentResource(source);
    resourceCache.set(hash, res);
  }
  return res;
}

export function getCachedFluentBundle(
  locale: string,
  ftlSource: string | readonly string[],
  options: CreateFluentBundleOptions = {}
): FluentBundle {
  const useIsolating = options.useIsolating ?? true;
  const hasCustomFunctions = options.functions && Object.keys(options.functions).length > 0;

  // We can only cache the whole bundle if there are no custom closures/functions
  const sourceHash = computeSourceHash(ftlSource);
  const cacheKey = `${locale}:iso=${useIsolating}:${sourceHash}`;

  if (!hasCustomFunctions && !options.disableCache) {
    const cached = bundleCache.get(cacheKey);
    if (cached) {
      return cached;
    }
  }

  const defaultFunctions = createDefaultFunctions(locale);
  const bundle = new FluentBundle(locale, {
    useIsolating,
    functions: {
      ...defaultFunctions,
      ...options.functions,
    },
  });

  const sources = Array.isArray(ftlSource) ? ftlSource : [ftlSource];
  for (const src of sources) {
    if (!src || typeof src !== 'string') continue;
    const resource = getOrCreateResource(src);
    const errors = bundle.addResource(resource, { allowOverrides: true });
    if (errors && errors.length > 0) {
      console.warn(`[next-fluent] Warnings adding FTL resource for locale ${locale}:`, errors);
    }
  }

  if (!hasCustomFunctions && !options.disableCache) {
    if (bundleCache.size >= MAX_BUNDLE_CACHE) {
      const keys = Array.from(bundleCache.keys()).slice(0, Math.floor(MAX_BUNDLE_CACHE * 0.2));
      for (const k of keys) bundleCache.delete(k);
    }
    bundleCache.set(cacheKey, bundle);
  }

  return bundle;
}

export function clearBundleCache(): void {
  resourceCache.clear();
  bundleCache.clear();
}

export function getBundleCacheStats(): { resourceCount: number; bundleCount: number } {
  return {
    resourceCount: resourceCache.size,
    bundleCount: bundleCache.size,
  };
}
