import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const apiBaseUrl = (process.env.RUSTOK_API_URL ?? "http://localhost:5150").replace(/\/$/, "");

/** @type {import('next').NextConfig} */
const nextConfig = {
  outputFileTracingRoot: join(__dirname, "../.."),
  reactStrictMode: true,
  // TypeScript 7 no longer exposes the legacy compiler API that Next uses
  // during `next build`; CI runs `npm run typecheck` as the canonical TS gate.
  typescript: {
    ignoreBuildErrors: true,
  },
  transpilePackages: [
    "@rustok/blog-frontend",
    "@rustok/comments-frontend",
    "@rustok/richtext",
  ],
  async rewrites() {
    return [
      {
        source: "/api/:path*",
        destination: `${apiBaseUrl}/api/:path*`,
      },
    ];
  },
  turbopack: {
    root: join(__dirname, "../.."),
  },
  webpack(config) {
    config.resolve.alias = {
      ...config.resolve.alias,
      "@": join(__dirname, "src"),
      "@rustok/next-fluent$": join(__dirname, "../../packages/next-fluent/dist/index.js"),
      "@rustok/next-fluent/client$": join(__dirname, "../../packages/next-fluent/dist/client.js"),
      "@rustok/next-fluent/server$": join(__dirname, "../../packages/next-fluent/dist/server.js"),
      "@rustok/next-fluent/middleware$": join(__dirname, "../../packages/next-fluent/dist/middleware.js"),
      "@rustok/next-fluent/typegen$": join(__dirname, "../../packages/next-fluent/dist/typegen.js")
    };
    return config;
  },
};

export default nextConfig;
