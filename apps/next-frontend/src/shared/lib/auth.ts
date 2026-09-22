import {
  getClientAuth,
  mapAuthError,
  type AuthError,
  type AuthSession,
  type AuthUser,
  ADMIN_TOKEN_KEY,
  ADMIN_TENANT_KEY,
  ADMIN_USER_KEY,
} from "@rustok/ui-auth/browser";

export {
  getClientAuth,
  mapAuthError,
  ADMIN_TOKEN_KEY,
  ADMIN_TENANT_KEY,
  ADMIN_USER_KEY,
};

export type { AuthError, AuthSession, AuthUser };
