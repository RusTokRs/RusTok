import createNextIntlPlugin from "next-intl/plugin";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const withNextIntl = createNextIntlPlugin();
const __dirname = dirname(fileURLToPath(import.meta.url));

/** @type {import('next').NextConfig} */
const nextConfig = {
  outputFileTracingRoot: join(__dirname, "../.."),
  reactStrictMode: true,
  transpilePackages: ["@rustok/blog-frontend"],
};

export default withNextIntl(nextConfig);
