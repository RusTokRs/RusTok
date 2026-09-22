import NextAuth from 'next-auth';
import Credentials from 'next-auth/providers/credentials';
import {
  signIn as rustokSignIn,
  fetchCurrentTenant
} from '@/shared/api/auth-api';

export const { handlers, signIn, signOut, auth } = NextAuth({
  providers: [
    Credentials({
      credentials: {
        email: { label: 'Email', type: 'email' },
        password: { label: 'Password', type: 'password' },
        tenantSlug: { label: 'Workspace', type: 'text' }
      },
      authorize: async (credentials) => {
        if (
          !credentials?.email ||
          !credentials?.password ||
          !credentials?.tenantSlug
        ) {
          return null;
        }
        try {
          const result = await rustokSignIn(
            credentials.email as string,
            credentials.password as string,
            credentials.tenantSlug as string
          );

          // Fetch tenantId via currentTenant query
          let tenantId: string | null = null;
          const tenant = await fetchCurrentTenant(
            result.accessToken,
            credentials.tenantSlug as string
          );
          if (tenant) {
            tenantId = tenant.id;
          }

          return {
            id: result.user.id,
            email: result.user.email,
            name: result.user.name,
            role: result.user.role,
            status: result.user.status,
            tenantSlug: credentials.tenantSlug as string,
            tenantId,
            rustokToken: result.accessToken
          };
        } catch {
          return null;
        }
      }
    })
  ],
  callbacks: {
    jwt({ token, user }) {
      if (user) {
        token.id = user.id ?? '';
        token.role = String(user.role ?? '');
        token.status = String(user.status ?? '');
        token.tenantSlug =
          typeof user.tenantSlug === 'string' || user.tenantSlug === null
            ? user.tenantSlug
            : null;
        token.tenantId =
          typeof user.tenantId === 'string' || user.tenantId === null
            ? user.tenantId
            : null;
        token.rustokToken = String(user.rustokToken ?? '');
      }
      return token;
    },
    session({ session, token }) {
      session.user.id = String(token.id ?? '');
      session.user.role = String(token.role ?? '');
      session.user.status = String(token.status ?? '');
      session.user.tenantSlug =
        typeof token.tenantSlug === 'string' || token.tenantSlug === null
          ? token.tenantSlug
          : null;
      session.user.tenantId =
        typeof token.tenantId === 'string' || token.tenantId === null
          ? token.tenantId
          : null;
      session.user.rustokToken = String(token.rustokToken ?? '');
      return session;
    }
  },
  pages: {
    signIn: '/auth/sign-in'
  },
  session: {
    strategy: 'jwt',
    maxAge: 7 * 24 * 60 * 60
  }
});
