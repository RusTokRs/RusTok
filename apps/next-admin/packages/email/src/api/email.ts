import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

// ---------- Types ----------

export interface EmailSettings {
  enabled: boolean;
  provider: string;
  from: string;
  resetBaseUrl: string;
  smtpHost: string;
  smtpPort: number;
  smtpUsername: string;
  smtpPassword: string;
}

export type EmailSettingsInput = Partial<EmailSettings>;

export const DEFAULT_EMAIL_SETTINGS: EmailSettings = {
  enabled: false,
  provider: 'smtp',
  from: 'noreply@example.com',
  resetBaseUrl: 'https://example.com',
  smtpHost: '',
  smtpPort: 587,
  smtpUsername: '',
  smtpPassword: ''
};

// ---------- GraphQL ----------

export const PLATFORM_SETTINGS_QUERY = `
query PlatformSettings($category: String!) {
  platformSettings(category: $category) {
    category
    settings
    schemaVersion
  }
}
`;

export const UPDATE_PLATFORM_SETTINGS_MUTATION = `
mutation UpdatePlatformSettings($input: UpdatePlatformSettingsInput!) {
  updatePlatformSettings(input: $input) {
    success
    category
    settings
    schemaVersion
  }
}
`;

export interface EmailSettingsSnapshot {
  settings: EmailSettings;
  schemaVersion: number;
}

interface PlatformSettingsResponse {
  platformSettings: { category: string; settings: string; schemaVersion: number };
}

interface UpdateSettingsResponse {
  updatePlatformSettings: {
    success: boolean;
    category: string;
    settings: string;
    schemaVersion: number;
  };
}

// ---------- API functions ----------

export async function getEmailSettings(
  opts: GqlOpts = {}
): Promise<EmailSettingsSnapshot> {
  const data = await graphqlRequest<
    { category: string },
    PlatformSettingsResponse
  >(
    PLATFORM_SETTINGS_QUERY,
    { category: 'email' },
    opts.token,
    opts.tenantSlug
  );

  const schemaVersion = data.platformSettings.schemaVersion;
  if (!Number.isInteger(schemaVersion) || schemaVersion < 0) {
    throw new TypeError('Platform settings response is missing a valid schema version');
  }
  try {
    const parsed = JSON.parse(
      data.platformSettings.settings ?? '{}'
    ) as Partial<EmailSettings>;
    return {
      settings: { ...DEFAULT_EMAIL_SETTINGS, ...parsed },
      schemaVersion
    };
  } catch {
    return { settings: { ...DEFAULT_EMAIL_SETTINGS }, schemaVersion };
  }
}

export async function updateEmailSettings(
  settings: EmailSettingsInput,
  expectedSchemaVersion: number,
  opts: GqlOpts = {}
): Promise<EmailSettingsSnapshot> {
  const data = await graphqlRequest<
    {
      input: {
        category: string;
        settings: string;
        expectedSchemaVersion: number;
      };
    },
    UpdateSettingsResponse
  >(
    UPDATE_PLATFORM_SETTINGS_MUTATION,
    {
      input: {
        category: 'email',
        settings: JSON.stringify(settings),
        expectedSchemaVersion
      }
    },
    opts.token,
    opts.tenantSlug
  );

  const schemaVersion = data.updatePlatformSettings.schemaVersion;
  if (!Number.isInteger(schemaVersion) || schemaVersion < 0) {
    throw new TypeError('Platform settings response is missing a valid schema version');
  }
  try {
    const parsed = JSON.parse(
      data.updatePlatformSettings.settings ?? '{}'
    ) as Partial<EmailSettings>;
    return {
      settings: { ...DEFAULT_EMAIL_SETTINGS, ...parsed },
      schemaVersion
    };
  } catch {
    return { settings: { ...DEFAULT_EMAIL_SETTINGS }, schemaVersion };
  }
}
