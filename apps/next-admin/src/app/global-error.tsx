'use client';

import NextError from 'next/error';
import { useEffect } from 'react';

async function captureException(error: Error) {
  if (process.env.NEXT_PUBLIC_SENTRY_DISABLED) return;

  const Sentry = await import('@sentry/nextjs');
  Sentry.captureException(error);
}

export default function GlobalError({
  error
}: {
  error: Error & { digest?: string };
}) {
  useEffect(() => {
    void captureException(error);
  }, [error]);

  return (
    <html>
      <body>
        {/* `NextError` is the default Next.js error page component. Its type
        definition requires a `statusCode` prop. However, since the App Router
        does not expose status codes for errors, we simply pass 0 to render a
        generic error message. */}
        <NextError statusCode={0} />
      </body>
    </html>
  );
}
