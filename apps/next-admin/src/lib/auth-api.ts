/**
 * Auth API functions — all communication goes through GraphQL.
 */

import { graphqlRequest } from './graphql';

// Types

export interface AuthUser {
  id: string;
  email: string;
  name: string | null;
  role: string;
  status: string;
}

export interface AuthSession {
  accessToken: string;
  refreshToken: string;
  tenantSlug: string | null;
}

export interface TenantInfo {
  id: string;
  name: string;
  slug: string;
}

// Mutations

const SIGN_IN_MUTATION = `
mutation SignIn($input: SignInInput!) {
  signIn(input: $input) {
    accessToken
    refreshToken
    tokenType
    expiresIn
    user {
      id
      email
      name
      role
      status
    }
  }
}
`;

const SIGN_UP_MUTATION = `
mutation SignUp($input: SignUpInput!) {
  signUp(input: $input) {
    accessToken
    refreshToken
    tokenType
    expiresIn
    user {
      id
      email
      name
      role
      status
    }
  }
}
`;

const SIGN_OUT_MUTATION = `
mutation SignOut {
  signOut {
    success
  }
}
`;

const CURRENT_USER_QUERY = `
query Me {
  me {
    id
    email
    name
    role
    status
  }
}
`;

const CURRENT_TENANT_QUERY = `
query CurrentTenant {
  currentTenant {
    id
    name
    slug
  }
}
`;

const REFRESH_TOKEN_MUTATION = `
mutation RefreshToken($input: RefreshTokenInput!) {
  refreshToken(input: $input) {
    accessToken
    refreshToken
    tokenType
    expiresIn
    user {
      id
      email
      name
      role
      status
    }
  }
}
`;

// Responses

interface AuthPayloadResponse {
  accessToken: string;
  refreshToken: string;
  tokenType: string;
  expiresIn: number;
  user: AuthUser;
}

interface SignInResponse {
  signIn: AuthPayloadResponse;
}

interface SignUpResponse {
  signUp: AuthPayloadResponse;
}

interface MeResponse {
  me: AuthUser | null;
}

interface CurrentTenantResponse {
  currentTenant: TenantInfo;
}

interface RefreshTokenResponse {
  refreshToken: AuthPayloadResponse;
}

// API functions

export async function signIn(
  email: string,
  password: string,
  tenantSlug: string
): Promise<{ accessToken: string; refreshToken: string; user: AuthUser }> {
  const data = await graphqlRequest<
    { input: { email: string; password: string } },
    SignInResponse
  >(SIGN_IN_MUTATION, { input: { email, password } }, undefined, tenantSlug);
  return {
    accessToken: data.signIn.accessToken,
    refreshToken: data.signIn.refreshToken,
    user: data.signIn.user
  };
}

export async function signUp(
  email: string,
  password: string,
  tenantSlug: string,
  name?: string
): Promise<{ accessToken: string; refreshToken: string; user: AuthUser }> {
  const data = await graphqlRequest<
    { input: { email: string; password: string; name?: string } },
    SignUpResponse
  >(SIGN_UP_MUTATION, { input: { email, password, name } }, undefined, tenantSlug);
  return {
    accessToken: data.signUp.accessToken,
    refreshToken: data.signUp.refreshToken,
    user: data.signUp.user
  };
}

export async function signOut(token: string, tenantSlug?: string | null): Promise<void> {
  try {
    await graphqlRequest(SIGN_OUT_MUTATION, undefined, token, tenantSlug);
  } catch {
    // Ignore sign out errors — clear local state regardless
  }
}

export async function fetchCurrentUser(
  token: string,
  tenantSlug?: string | null
): Promise<AuthUser | null> {
  try {
    const data = await graphqlRequest<undefined, MeResponse>(
      CURRENT_USER_QUERY,
      undefined,
      token,
      tenantSlug
    );
    return data.me;
  } catch {
    return null;
  }
}

export async function fetchCurrentTenant(
  token: string,
  tenantSlug?: string | null
): Promise<TenantInfo | null> {
  try {
    const data = await graphqlRequest<undefined, CurrentTenantResponse>(
      CURRENT_TENANT_QUERY,
      undefined,
      token,
      tenantSlug
    );
    return data.currentTenant;
  } catch {
    return null;
  }
}

export async function refreshToken(
  currentRefreshToken: string,
  tenantSlug?: string | null
): Promise<{ accessToken: string; refreshToken: string; user: AuthUser }> {
  const data = await graphqlRequest<
    { input: { refreshToken: string } },
    RefreshTokenResponse
  >(REFRESH_TOKEN_MUTATION, { input: { refreshToken: currentRefreshToken } }, undefined, tenantSlug);
  return {
    accessToken: data.refreshToken.accessToken,
    refreshToken: data.refreshToken.refreshToken,
    user: data.refreshToken.user
  };
}
