export type AppType =
  'Embedded' | 'FirstParty' | 'Mobile' | 'Service' | 'ThirdParty';

export interface OAuthApp {
  id: string;
  name: string;
  slug: string;
  description?: string;
  iconUrl?: string;
  appType: AppType;
  clientId: string;
  redirectUris: string[];
  scopes: string[];
  grantTypes: string[];
  manifestRef?: string;
  autoCreated: boolean;
  managedByManifest: boolean;
  isActive: boolean;
  canEdit: boolean;
  canRotateSecret: boolean;
  canRevoke: boolean;
  activeTokenCount: number;
  lastUsedAt?: string;
  createdAt: string;
}

export type ManualAppType = 'ThirdParty' | 'Mobile' | 'Service';

export interface CreateOAuthAppInput {
  name: string;
  slug: string;
  description?: string;
  iconUrl?: string;
  appType: ManualAppType;
  redirectUris?: string[];
  scopes: string[];
  grantTypes: string[];
}

export interface UpdateOAuthAppInput {
  name: string;
  description?: string;
  iconUrl?: string;
  redirectUris: string[];
  scopes: string[];
  grantTypes: string[];
}

export interface CreateOAuthAppResult {
  app: OAuthApp;
  clientSecret: string;
}
