'use client';

import { useRef, useState } from 'react';
import { useTranslations } from '@rustok/next-fluent/client';
import { toast } from 'sonner';
import { requestConfirmedLogout } from '@/shared/api/auth-session';

/** Capture the displayed lineage, not a newly fetched identity. A stale
 * click must never revoke a new login, even for the same subject.
 */
export function useConfirmedSignOut(expectedSessionId: string | null | undefined) {
  const t = useTranslations('app.menu');
  const inFlight = useRef(false);
  const [isSigningOut, setIsSigningOut] = useState(false);

  async function signOut(): Promise<boolean> {
    if (inFlight.current) return false;
    inFlight.current = true;
    setIsSigningOut(true);
    try {
      await requestConfirmedLogout(expectedSessionId);
      // The same response clears both cookie sets. A full navigation drops
      // stale client caches; never invoke a second Auth.js signout request.
      window.location.replace('/auth/sign-in');
      return true;
    } catch {
      toast.error(t('signOutFailed'));
      return false;
    } finally {
      inFlight.current = false;
      setIsSigningOut(false);
    }
  }

  return { signOut, isSigningOut };
}
