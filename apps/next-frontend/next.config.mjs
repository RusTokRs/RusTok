import createNextIntlPlugin from "next-intl/plugin";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const withNextIntl = createNextIntlPlugin();
const __dirname = dirname(fileURLToPath(import.meta.url));

/** @type {import('next').NextConfig} */
const nextConfig = {
  outputFileTracingRoot: join(__dirname, "../.."),
  reactStrictMode: true,
  // TypeScript 7 no longer exposes the legacy compiler API that Next uses
  // during `next build`; CI runs `npm run typecheck` as the canonical TS gate.
  typescript: {
    ignoreBuildErrors: true,
  },
  transpilePackages: ["@rustok/blog-frontend"],
  webpack(config) {
    config.resolve.alias = {
      ...config.resolve.alias,
      "@": join(__dirname, "src"),
    };
    return config;
  },
};

export default withNextIntl(nextConfig);
