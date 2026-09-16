import { withSentryConfig } from '@sentry/nextjs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/** @type {import('next').NextConfig} */
const baseConfig = {
  // TypeScript 7 no longer exposes the legacy compiler API that Next uses
  // during `next build`; CI runs `npm run typecheck` as the canonical TS gate.
  typescript: {
    ignoreBuildErrors: true
  },
  async rewrites() {
    const apiBaseUrl =
      process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:5150';
    return [
      {
        source: '/api/graphql',
        destination: `${apiBaseUrl}/api/graphql`
      }
    ];
  },
  images: {
    remotePatterns: [
      {
        protocol: 'https',
        hostname: 'api.slingacademy.com',
        port: ''
      }
    ]
  },
  transpilePackages: [
    'geist',
    '@rustok/blog-admin',
    '@rustok/ai-admin',
    '@rustok/commerce-admin',
    '@rustok/events-admin',
    '@rustok/iggy-connector-admin',
    '@rustok/richtext'
  ],
  // Turbopack configuration: set workspace root and resolveAlias so local crate packages
  // (e.g. @rustok/events-admin at crates/modules/...) can resolve @rustok/next-fluent
  turbopack: {
    root: path.resolve(__dirname, '../..')
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
      '@rustok/next-fluent$': path.resolve(__dirname, '../../packages/next-fluent/dist/index.js'),
      '@rustok/next-fluent/client$': path.resolve(__dirname, '../../packages/next-fluent/dist/client.js'),
      '@rustok/next-fluent/server$': path.resolve(__dirname, '../../packages/next-fluent/dist/server.js'),
      '@rustok/next-fluent/middleware$': path.resolve(__dirname, '../../packages/next-fluent/dist/middleware.js'),
      '@rustok/next-fluent/typegen$': path.resolve(__dirname, '../../packages/next-fluent/dist/typegen.js')
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

    webpack: {
      reactComponentAnnotation: {
        enabled: true
      },
      treeshake: {
        removeDebugLogging: true
      }
    },

    // Route browser requests to Sentry through a Next.js rewrite to circumvent ad-blockers.
    // This can increase your server load as well as your hosting bill.
    // Note: Check that the configured route will not match with your Next.js middleware, otherwise reporting of client-
    // side errors will fail.
    tunnelRoute: '/monitoring',

    // Disable Sentry telemetry
    telemetry: false
  });
}

export default configWithPlugins;
