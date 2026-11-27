export const ADMIN_TOKEN_KEY = "rustok-admin-token";
export const ADMIN_TENANT_KEY = "rustok-admin-tenant";
export const ADMIN_USER_KEY = "rustok-admin-user";

export type AuthUser = {
  id: string;
  email: string;
  name?: string | null;
  role: string;
  /**
   * Effective RBAC permission snapshot (`"{resource}:{action}"` strings,
   * e.g. `pages:update`) as resolved by the backend auth service.
   * Capability decisions must read this list; `role` is for display only.
   */
  permissions?: string[];
};

export type AuthSession = {
  token: string;
  tenant: string;
  expiresAt?: number;
};

const REFRESH_SAFETY_WINDOW_SECS = 60;

export function secsUntilExpiry(expiresAt: number, nowSecs = Math.floor(Date.now() / 1000)): number {
  return Math.max(0, expiresAt - nowSecs);
}

export function isSessionExpired(expiresAt: number, nowSecs = Math.floor(Date.now() / 1000)): boolean {
  return nowSecs >= expiresAt - REFRESH_SAFETY_WINDOW_SECS;
}

export type AuthError =
  | { type: "unauthorized" }
  | { type: "invalid_credentials" }
  | { type: "network" }
  | { type: "http"; status: number };

export function mapAuthError(status: number, isLogin: boolean): AuthError {
  if (status === 401 && isLogin) {
    return { type: "invalid_credentials" };
  }
  if (status === 401) {
    return { type: "unauthorized" };
  }
  return { type: "http", status };
}

export function getCookieValue(name: string, cookieSource?: string) {
  const source = cookieSource ?? document.cookie;
  const prefix = `${name}=`;
  const pair = source
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix));

  if (!pair) {
    return undefined;
  }

  const encodedValue = pair.slice(prefix.length);
  try {
    return decodeURIComponent(encodedValue);
  } catch {
    return undefined;
  }
}

export function getClientAuth(cookieSource?: string): AuthSession {
  return {
    token: getCookieValue(ADMIN_TOKEN_KEY, cookieSource) ?? "",
    tenant: getCookieValue(ADMIN_TENANT_KEY, cookieSource) ?? "",
  };
}
