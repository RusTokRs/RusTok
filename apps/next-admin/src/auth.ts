import NextAuth from 'next-auth';
import Credentials from 'next-auth/providers/credentials';
import { fetchCurrentTenant } from '@/shared/api/tenant-api';
import { handleConfirmedLogout } from '@/shared/api/auth-session';
import {
  clearRustokSessionCookies,
  createRustokSession,
  persistRustokSessionCookies,
  readRustokSessionCookies,
  revokeRustokSession
} from '@/shared/api/auth-server';

function asString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

const { handlers, signIn, signOut: clearAuthJsSession, auth } = NextAuth({
  providers: [
    Credentials({
      credentials: {
        action: { label: 'Action', type: 'text' },
        email: { label: 'Email', type: 'email' },
        password: { label: 'Password', type: 'password' },
        name: { label: 'Name', type: 'text' },
        tenantSlug: { label: 'Workspace', type: 'text' }
      },
      authorize: async (credentials, request) => {
        const rawAction = asString(credentials?.action);
        if (
          rawAction &&
          rawAction !== 'login' &&
          rawAction !== 'register'
        ) {
          return null;
        }

        const action = rawAction === 'register' ? 'register' : 'login';
        const email = asString(credentials?.email)?.trim() ?? '';
        const password = asString(credentials?.password) ?? '';
        const tenantSlug = asString(credentials?.tenantSlug)?.trim() ?? '';
        const name = asString(credentials?.name)?.trim() || undefined;

        if (!email || !password || !tenantSlug) return null;

        let issued: Awaited<
          ReturnType<typeof createRustokSession>
        > | null = null;

        try {
          issued = await createRustokSession({
            action,
            email,
            password,
            tenantSlug,
            name,
            request
          });

          const tenant = await fetchCurrentTenant(
            issued.accessToken,
            tenantSlug
          );
          if (tenant && tenant.id !== issued.tenantId) throw new Error('Tenant identity mismatch');
          await persistRustokSessionCookies(issued);

          return {
            id: issued.user.id,
            email: issued.user.email,
            name: issued.user.name,
            role: issued.user.role,
            status: issued.user.status,
            tenantSlug,
            tenantId: issued.tenantId,
            rustokSessionId: issued.sessionId
          };
        } catch {
          if (issued) {
            try {
              await revokeRustokSession(issued.refreshCredential, tenantSlug, issued.sessionId, issued.tenantId);
            } catch {
              console.warn(
                'Rustok session cleanup failed after credential establishment'
              );
            }
          }

          // A rejected login must not clear a pre-existing or concurrently
          // established session. Only the newly issued lineage is compensable.
          return null;
        }
      }
    })
  ],
  callbacks: {
    jwt({ token, user, trigger, session }) {
      if (user) {
        token.id = user.id ?? '';
        token.role = String(user.role ?? '');
        token.status = String(user.status ?? '');
        token.tenantSlug =
          typeof user.tenantSlug === 'string' ? user.tenantSlug : null;
        token.tenantId =
          typeof user.tenantId === 'string' ? user.tenantId : null;
        token.rustokSessionId = typeof user.rustokSessionId === 'string' ? user.rustokSessionId : null;
      }

      if (
        trigger === 'update' &&
        session &&
        typeof (session as { name?: unknown }).name === 'string'
      ) {
        token.name = (session as { name: string }).name;
      }

      return token;
    },
    session({ session, token }) {
      session.user.id = String(token.id ?? '');
      session.user.role = String(token.role ?? '');
      session.user.status = String(token.status ?? '');
      session.user.tenantSlug =
        typeof token.tenantSlug === 'string' ? token.tenantSlug : null;
      session.user.tenantId =
        typeof token.tenantId === 'string' ? token.tenantId : null;
      session.user.rustokSessionId =
        typeof token.rustokSessionId === 'string' ? token.rustokSessionId : null;
      if (typeof token.name === 'string') {
        session.user.name = token.name;
      }
      return session;
    }
  },
  pages: {
    signIn: '/auth/sign-in'
  },
  session: {
    strategy: 'jwt',
    maxAge: 30 * 24 * 60 * 60
  }
});

export { handlers, signIn, auth };

// The raw Auth.js signOut helper stays private. The route admits the caller
// before any mutation; Auth.js cleanup runs only after owner confirmation.
export async function confirmedSignOut(request: Request): Promise<Response> {
  return handleConfirmedLogout(request, {
    async readIdentity() {
      const session = await auth();
      return session?.user ? {
        sessionId: session.user.rustokSessionId,
        tenantId: session.user.tenantId,
        tenantSlug: session.user.tenantSlug
      } : null;
    },
    readCookies: readRustokSessionCookies,
    revoke: revokeRustokSession,
    async clearApplicationSession() {
      const target = await clearAuthJsSession({ redirect: false, redirectTo: '/auth/sign-in' });
      // The supported server helper returns the target URL when redirect is
      // false. An Auth.js error redirect is not local cleanup confirmation.
      if (typeof target !== 'string' ||
          new URL(target, request.url).href !== new URL('/auth/sign-in', request.url).href) {
        throw new Error('Auth.js sign-out was not confirmed');
      }
    },
    clearRustokCookies: clearRustokSessionCookies
  });
}
