import type * as Sentry from '@sentry/nextjs';

const sentryOptions: Sentry.NodeOptions | Sentry.EdgeOptions = {
  // Sentry DSN
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,

  // Enable Spotlight in development
  spotlight: process.env.NODE_ENV === 'development',

  // Adds request headers and IP for users, for more info visit
  sendDefaultPii: true,

  // Adjust this value in production, or use tracesSampler for greater control
  tracesSampleRate: 1,

  // Setting this option to true will print useful information to the console while you're setting up Sentry.
  debug: false
};

function isSentryEnabled() {
  return !process.env.NEXT_PUBLIC_SENTRY_DISABLED;
}

async function loadSentry() {
  return import('@sentry/nextjs');
}

export async function register() {
  if (!isSentryEnabled()) return;

  const SentryRuntime = await loadSentry();

  if (process.env.NEXT_RUNTIME === 'nodejs') {
    // Node.js Sentry configuration
    SentryRuntime.init(sentryOptions);
  }

  if (process.env.NEXT_RUNTIME === 'edge') {
    // Edge Sentry configuration
    SentryRuntime.init(sentryOptions);
  }
}

export const onRequestError = async (
  ...args: Parameters<typeof import('@sentry/nextjs').captureRequestError>
) => {
  if (!isSentryEnabled()) return;

  const SentryRuntime = await loadSentry();
  return SentryRuntime.captureRequestError(...args);
};
