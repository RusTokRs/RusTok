import NextAuth from 'next-auth';
import Credentials from 'next-auth/providers/credentials';
import { fetchCurrentTenant } from '@/shared/api/tenant-api';
import {
  clearRustokSessionCookies,
  createRustokSession,
  persistRustokSessionCookies,
  readRustokSessionCookies,
  revokeRustokSession
} from '@/shared/api/auth-server';

const asString = (value: unknown) => typeof value === 'string' ? value : null;

export const { handlers, signIn, signOut, auth } = NextAuth({
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
        if (rawAction && rawAction !== 'login' && rawAction !== 'register') return null;
        const action = rawAction === 'register' ? 'register' : 'login';
        const email = asString(credentials?.email)?.trim() ?? '';
        const password = asString(credentials?.password) ?? '';
        const tenantSlug = asString(credentials?.tenantSlug)?.trim() ?? '';
        const name = asString(credentials?.name)?.trim() || undefined;
        if (!email || !password || !tenantSlug) return null;

        let issued: Awaited<ReturnType<typeof createRustokSession>> | null = null;
        try {
          issued = await createRustokSession({ action, email, password, tenantSlug, name, request });
          const tenant = await fetchCurrentTenant(issued.accessToken, tenantSlug);
          await persistRustokSessionCookies(issued);
          return {
            id: issued.user.id,
            email: issued.user.email,
            name: issued.user.name,
            role: issued.user.role,
            status: issued.user.status,
            tenantSlug,
            tenantId: tenant?.id ?? null
          };
        } catch {
          if (issued) {
            try { await revokeRustokSession(issued.refreshToken, tenantSlug); }
            catch { console.warn('Rustok session cleanup failed after credential establishment'); }
          }
          await clearRustokSessionCookies().catch(() => {
            console.warn('Rustok session cookie cleanup failed');
          });
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
        token.tenantSlug = typeof user.tenantSlug === 'string' ? user.tenantSlug : null;
        token.tenantId = typeof user.tenantId === 'string' ? user.tenantId : null;
      }
      if (trigger === 'update' && session && typeof (session as {name?: unknown}).name === 'string') {
        token.name = (session as {name: string}).name;
      }
      return token;
    },
    session({ session, token }) {
      session.user.id = String(token.id ?? '');
      session.user.role = String(token.role ?? '');
      session.user.status = String(token.status ?? '');
      session.user.tenantSlug = typeof token.tenantSlug === 'string' ? token.tenantSlug : null;
      session.user.tenantId = typeof token.tenantId === 'string' ? token.tenantId : null;
      if (typeof token.name === 'string') session.user.name = token.name;
      return session;
    }
  },
  events: {
    async signOut(message) {
      const token = 'token' in message ? message.token : null;
      const tenantSlug = typeof token?.tenantSlug === 'string' ? token.tenantSlug : null;
      const tenantId = typeof token?.tenantId === 'string' ? token.tenantId : null;
      try {
        const stored = await readRustokSessionCookies();
        if (stored.refreshToken && tenantSlug) {
          await revokeRustokSession(stored.refreshToken, tenantSlug, tenantId);
        }
      } catch {
        console.warn('Rustok server session revocation failed during sign-out');
      } finally {
        await clearRustokSessionCookies().catch(() => {
          console.warn('Rustok session cookie cleanup failed during sign-out');
        });
      }
    }
  },
  pages: { signIn: '/auth/sign-in' },
  session: { strategy: 'jwt', maxAge: 30 * 24 * 60 * 60 }
});
