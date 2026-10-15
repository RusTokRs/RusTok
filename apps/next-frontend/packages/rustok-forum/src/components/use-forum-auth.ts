'use client';

import { useEffect, useState } from 'react';
import { getClientAuth, type AuthSession } from '@/shared/lib/auth';

/** Request-bound browser auth. Never pass this value as a server component prop. */
export function useForumAuth(): AuthSession | null {
  const [auth, setAuth] = useState<AuthSession | null>(null);

  useEffect(() => {
    const sync = () => {
      const current = getClientAuth();
      setAuth((previous) =>
        previous?.token === current.token && previous.tenant === current.tenant
          ? previous
          : current
      );
    };
    const onVisible = () => { if (!document.hidden) sync(); };
    sync();
    window.addEventListener('focus', sync);
    window.addEventListener('pageshow', sync);
    window.addEventListener('storage', sync);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      window.removeEventListener('focus', sync);
      window.removeEventListener('pageshow', sync);
      window.removeEventListener('storage', sync);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, []);

  return auth;
}
