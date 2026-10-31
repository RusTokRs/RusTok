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
  }
}
`;

export const UPDATE_PLATFORM_SETTINGS_MUTATION = `
mutation UpdatePlatformSettings($input: UpdatePlatformSettingsInput!) {
  updatePlatformSettings(input: $input) {
    success
    category
    settings
  }
}
`;

interface PlatformSettingsResponse {
  platformSettings: { category: string; settings: string };
}

interface UpdateSettingsResponse {
  updatePlatformSettings: {
    success: boolean;
    category: string;
    settings: string;
  };
}

// ---------- API functions ----------

export async function getEmailSettings(
  opts: GqlOpts = {}
): Promise<EmailSettings> {
  const data = await graphqlRequest<
    { category: string },
    PlatformSettingsResponse
  >(
    PLATFORM_SETTINGS_QUERY,
    { category: 'email' },
    opts.token,
    opts.tenantSlug
  );

  try {
    const parsed = JSON.parse(
      data?.platformSettings?.settings ?? '{}'
    ) as Partial<EmailSettings>;
    return {
      ...DEFAULT_EMAIL_SETTINGS,
      ...parsed
    };
  } catch {
    return DEFAULT_EMAIL_SETTINGS;
  }
}

export async function updateEmailSettings(
  settings: EmailSettingsInput,
  opts: GqlOpts = {}
): Promise<EmailSettings> {
  const data = await graphqlRequest<
    { input: { category: string; settings: string } },
    UpdateSettingsResponse
  >(
    UPDATE_PLATFORM_SETTINGS_MUTATION,
    { input: { category: 'email', settings: JSON.stringify(settings) } },
    opts.token,
    opts.tenantSlug
  );

  try {
    const parsed = JSON.parse(
      data?.updatePlatformSettings?.settings ?? '{}'
    ) as Partial<EmailSettings>;
    return {
      ...DEFAULT_EMAIL_SETTINGS,
      ...parsed
    };
  } catch {
    return DEFAULT_EMAIL_SETTINGS;
  }
}
