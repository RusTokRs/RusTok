export const ADMIN_TOKEN_KEY = "rustok-admin-token";
export const ADMIN_TENANT_KEY = "rustok-admin-tenant";
export const ADMIN_USER_KEY = "rustok-admin-user";

export type AuthUser = {
  id: string;
  email: string;
  name?: string | null;
  role: string;
};

export type AuthSession = {
  token: string;
  tenant: string;
};

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
    tenant: getCookieValue(ADMIN_TENANT_KEY, cookieSource) ?? "demo",
  };
}
