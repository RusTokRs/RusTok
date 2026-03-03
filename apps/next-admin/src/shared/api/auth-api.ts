export {
  signIn,
  signUp,
  signOut,
  fetchCurrentUser,
  fetchCurrentTenant,
  refreshToken
} from '@/lib/auth-api';
export type { AuthUser, AuthSession, TenantInfo } from '@/lib/auth-api';
