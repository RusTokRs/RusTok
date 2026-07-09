import type {
  CreateOAuthAppInput,
  CreateOAuthAppResult,
  OAuthApp,
  UpdateOAuthAppInput
} from '@/entities/oauth-app';
import { graphqlRequest } from './graphql';

const OAUTH_APP_FIELDS = `
  id
  name
  slug
  description
  iconUrl
  appType
  clientId
  redirectUris
  scopes
  grantTypes
  manifestRef
  autoCreated
  managedByManifest
  isActive
  canEdit
  canRotateSecret
  canRevoke
  activeTokenCount
  lastUsedAt
  createdAt
`;

const OAUTH_APPS_QUERY = `
query OAuthApps($limit: Int) {
  oauthApps(limit: $limit) {
    ${OAUTH_APP_FIELDS}
  }
}
`;

const CREATE_OAUTH_APP_MUTATION = `
mutation CreateOAuthApp($input: CreateOAuthAppInput!) {
  createOAuthApp(input: $input) {
    app {
      ${OAUTH_APP_FIELDS}
    }
    clientSecret
  }
}
`;

const UPDATE_OAUTH_APP_MUTATION = `
mutation UpdateOAuthApp($id: UUID!, $input: UpdateOAuthAppInput!) {
  updateOAuthApp(id: $id, input: $input) {
    ${OAUTH_APP_FIELDS}
  }
}
`;

const ROTATE_SECRET_MUTATION = `
mutation RotateOAuthAppSecret($id: UUID!) {
  rotateOAuthAppSecret(id: $id) {
    app {
      ${OAUTH_APP_FIELDS}
    }
    clientSecret
  }
}
`;

const REVOKE_APP_MUTATION = `
mutation RevokeOAuthApp($id: UUID!) {
  revokeOAuthApp(id: $id) {
    id
  }
}
`;

interface OAuthAppGraphql {
  id: string;
  name: string;
  slug: string;
  description?: string | null;
  iconUrl?: string | null;
  appType: string;
  clientId: string;
  redirectUris: string[];
  scopes: string[];
  grantTypes: string[];
  manifestRef?: string | null;
  autoCreated: boolean;
  managedByManifest: boolean;
  isActive: boolean;
  canEdit: boolean;
  canRotateSecret: boolean;
  canRevoke: boolean;
  activeTokenCount: number;
  lastUsedAt?: string | null;
  createdAt: string;
}

interface OAuthAppsResponse {
  oauthApps: OAuthAppGraphql[];
}

interface CreateOAuthAppResponse {
  createOAuthApp: {
    app: OAuthAppGraphql;
    clientSecret: string;
  };
}

interface UpdateOAuthAppResponse {
  updateOAuthApp: OAuthAppGraphql;
}

interface RotateSecretResponse {
  rotateOAuthAppSecret: {
    app: OAuthAppGraphql;
    clientSecret: string;
  };
}

function mapAppType(appType: string): OAuthApp['appType'] {
  switch (appType) {
    case 'EMBEDDED':
      return 'Embedded';
    case 'FIRST_PARTY':
      return 'FirstParty';
    case 'MOBILE':
      return 'Mobile';
    case 'SERVICE':
      return 'Service';
    case 'THIRD_PARTY':
    default:
      return 'ThirdParty';
  }
}

function toGraphqlAppType(appType: CreateOAuthAppInput['appType']): string {
  switch (appType) {
    case 'Mobile':
      return 'MOBILE';
    case 'Service':
      return 'SERVICE';
    case 'ThirdParty':
    default:
      return 'THIRD_PARTY';
  }
}

function mapOAuthApp(app: OAuthAppGraphql): OAuthApp {
  return {
    id: app.id,
    name: app.name,
    slug: app.slug,
    description: app.description ?? undefined,
    iconUrl: app.iconUrl ?? undefined,
    appType: mapAppType(app.appType),
    clientId: app.clientId,
    redirectUris: app.redirectUris,
    scopes: app.scopes,
    grantTypes: app.grantTypes,
    manifestRef: app.manifestRef ?? undefined,
    autoCreated: app.autoCreated,
    managedByManifest: app.managedByManifest,
    isActive: app.isActive,
    canEdit: app.canEdit,
    canRotateSecret: app.canRotateSecret,
    canRevoke: app.canRevoke,
    activeTokenCount: app.activeTokenCount,
    lastUsedAt: app.lastUsedAt ?? undefined,
    createdAt: app.createdAt
  };
}

export async function listOAuthApps(
  token: string,
  tenantSlug?: string | null
): Promise<OAuthApp[]> {
  const data = await graphqlRequest<{ limit: number }, OAuthAppsResponse>(
    OAUTH_APPS_QUERY,
    { limit: 100 },
    token,
    tenantSlug
  );

  return data.oauthApps.map(mapOAuthApp);
}

export async function createOAuthApp(
  token: string,
  tenantSlug: string | null | undefined,
  input: CreateOAuthAppInput
): Promise<CreateOAuthAppResult> {
  const data = await graphqlRequest<
    {
      input: {
        name: string;
        slug: string;
        description?: string;
        iconUrl?: string;
        appType: string;
        redirectUris?: string[];
        scopes: string[];
        grantTypes: string[];
      };
    },
    CreateOAuthAppResponse
  >(
    CREATE_OAUTH_APP_MUTATION,
    {
      input: {
        name: input.name,
        slug: input.slug,
        description: input.description,
        iconUrl: input.iconUrl,
        appType: toGraphqlAppType(input.appType),
        redirectUris: input.redirectUris,
        scopes: input.scopes,
        grantTypes: input.grantTypes
      }
    },
    token,
    tenantSlug
  );

  return {
    app: mapOAuthApp(data.createOAuthApp.app),
    clientSecret: data.createOAuthApp.clientSecret
  };
}

export async function updateOAuthApp(
  token: string,
  tenantSlug: string | null | undefined,
  id: string,
  input: UpdateOAuthAppInput
): Promise<OAuthApp> {
  const data = await graphqlRequest<
    {
      id: string;
      input: {
        name: string;
        description?: string;
        iconUrl?: string;
        redirectUris: string[];
        scopes: string[];
        grantTypes: string[];
      };
    },
    UpdateOAuthAppResponse
  >(
    UPDATE_OAUTH_APP_MUTATION,
    {
      id,
      input: {
        name: input.name,
        description: input.description,
        iconUrl: input.iconUrl,
        redirectUris: input.redirectUris,
        scopes: input.scopes,
        grantTypes: input.grantTypes
      }
    },
    token,
    tenantSlug
  );

  return mapOAuthApp(data.updateOAuthApp);
}

export async function rotateOAuthAppSecret(
  token: string,
  tenantSlug: string | null | undefined,
  id: string
): Promise<CreateOAuthAppResult> {
  const data = await graphqlRequest<{ id: string }, RotateSecretResponse>(
    ROTATE_SECRET_MUTATION,
    { id },
    token,
    tenantSlug
  );

  return {
    app: mapOAuthApp(data.rotateOAuthAppSecret.app),
    clientSecret: data.rotateOAuthAppSecret.clientSecret
  };
}

export async function revokeOAuthApp(
  token: string,
  tenantSlug: string | null | undefined,
  id: string
): Promise<void> {
  await graphqlRequest<{ id: string }, { revokeOAuthApp: { id: string } }>(
    REVOKE_APP_MUTATION,
    { id },
    token,
    tenantSlug
  );
}
