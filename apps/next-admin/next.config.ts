import type { NextConfig } from 'next';
import { withSentryConfig } from '@sentry/nextjs';
import createNextIntlPlugin from 'next-intl/plugin';

import path from 'path';

// Define the base Next.js configuration
const baseConfig: NextConfig = {
  images: {
    remotePatterns: [
      {
        protocol: 'https',
        hostname: 'api.slingacademy.com',
        port: ''
      }
    ]
  },
  transpilePackages: ['geist', '@rustok/blog-admin', '@rustok/ai-admin'],
  // Turbopack configuration: set workspace root so local crate packages
  // (e.g. @rustok/blog-admin at file:../../crates/...) can resolve node_modules
  // from the workspace junction at the repo root.
  turbopack: {
    root: path.resolve(__dirname, '../..'),
  },
  experimental: {
    turbopackUseSystemTlsCerts: true,
  },
  webpack(config) {
    // Allow @rustok/blog-admin (and other local crate UI packages) to resolve
    // the host application's path aliases (@/*, @/shared/*, etc.) so they can
    // import shared UI components without duplicating them in each package.
    config.resolve.alias = {
      ...config.resolve.alias,
      '@': path.resolve(__dirname, 'src'),
      '@/shared': path.resolve(__dirname, 'src/shared'),
      '@/entities': path.resolve(__dirname, 'src/entities'),
      '@/widgets': path.resolve(__dirname, 'src/widgets'),
      '@/modules': path.resolve(__dirname, 'src/modules'),
      '@/types': path.resolve(__dirname, 'src/types'),
      '@/lib': path.resolve(__dirname, 'src/lib'),
      '@/components': path.resolve(__dirname, 'src/components'),
      '@/config': path.resolve(__dirname, 'src/config'),
      '@/constants': path.resolve(__dirname, 'src/constants'),
      '@/hooks': path.resolve(__dirname, 'src/hooks'),
    };
    return config;
  }
};

let configWithPlugins = baseConfig;

// Conditionally enable Sentry configuration
if (!process.env.NEXT_PUBLIC_SENTRY_DISABLED) {
  configWithPlugins = withSentryConfig(configWithPlugins, {
    // For all available options, see:
    // https://www.npmjs.com/package/@sentry/webpack-plugin#options
    // FIXME: Add your Sentry organization and project names
    org: process.env.NEXT_PUBLIC_SENTRY_ORG,
    project: process.env.NEXT_PUBLIC_SENTRY_PROJECT,
    // Only print logs for uploading source maps in CI
    silent: !process.env.CI,

    // For all available options, see:
    // https://docs.sentry.io/platforms/javascript/guides/nextjs/manual-setup/

    // Upload a larger set of source maps for prettier stack traces (increases build time)
    widenClientFileUpload: true,

    // Upload a larger set of source maps for prettier stack traces (increases build time)
    reactComponentAnnotation: {
      enabled: true
    },

    // Route browser requests to Sentry through a Next.js rewrite to circumvent ad-blockers.
    // This can increase your server load as well as your hosting bill.
    // Note: Check that the configured route will not match with your Next.js middleware, otherwise reporting of client-
    // side errors will fail.
    tunnelRoute: '/monitoring',

    // Automatically tree-shake Sentry logger statements to reduce bundle size
    disableLogger: true,

    // Disable Sentry telemetry
    telemetry: false
  });
}

const withNextIntl = createNextIntlPlugin('./src/i18n/request.ts');
const nextConfig = withNextIntl(configWithPlugins);
export default nextConfig;
