import { DefaultSession } from 'next-auth';

declare module 'next-auth' {
  interface Session {
    user: {
      id: string;
      role: string;
      status: string;
      tenantSlug: string | null;
      tenantId: string | null;
      rustokToken: string;
    } & DefaultSession['user'];
  }

  interface User {
    role: string;
    status: string;
    tenantSlug: string | null;
    tenantId: string | null;
    rustokToken: string;
  }
}

declare module 'next-auth/jwt' {
  interface JWT {
    id: string;
    role: string;
    status: string;
    tenantSlug: string | null;
    tenantId: string | null;
    rustokToken: string;
  }
}
