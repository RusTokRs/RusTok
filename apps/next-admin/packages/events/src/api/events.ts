import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

export interface EventsStatus {
  configuredTransport: string;
  iggyMode: string;
  relayIntervalMs: number;
  dlqEnabled: boolean;
  maxAttempts: number;
  pendingEvents: number;
  dlqEvents: number;
  availableTransports: string[];
}

export interface EventsSettings {
  transport: string;
  relay_interval_ms: number;
  max_attempts: number;
  dlq_enabled: boolean;
  iggy_addresses: string;
  iggy_protocol: string;
  iggy_username: string;
  iggy_password: string;
  iggy_tls: boolean;
  iggy_stream: string;
  iggy_partitions: number;
  iggy_replication: number;
}

const EVENTS_STATUS_QUERY = `
query EventsStatus {
  eventsStatus {
    configuredTransport iggyMode relayIntervalMs dlqEnabled maxAttempts
    pendingEvents dlqEvents availableTransports
  }
}`;

const PLATFORM_SETTINGS_QUERY = `
query PlatformSettings($category: String!) {
  platformSettings(category: $category) { category settings }
}`;

const UPDATE_PLATFORM_SETTINGS_MUTATION = `
mutation UpdatePlatformSettings($input: UpdatePlatformSettingsInput!) {
  updatePlatformSettings(input: $input) { success category }
}`;

export async function getEventsStatus(
  opts: GqlOpts = {}
): Promise<EventsStatus> {
  const data = await graphqlRequest<
    Record<string, never>,
    { eventsStatus: EventsStatus }
  >(EVENTS_STATUS_QUERY, {}, opts.token, opts.tenantSlug);
  return data.eventsStatus;
}

export async function getEventsSettings(
  opts: GqlOpts = {}
): Promise<EventsSettings | null> {
  try {
    const data = await graphqlRequest<
      { category: string },
      { platformSettings: { settings: string } }
    >(
      PLATFORM_SETTINGS_QUERY,
      { category: 'events' },
      opts.token,
      opts.tenantSlug
    );
    return JSON.parse(data.platformSettings.settings) as EventsSettings;
  } catch {
    return null;
  }
}

export async function saveEventsSettings(
  settings: EventsSettings,
  opts: GqlOpts = {}
): Promise<boolean> {
  const data = await graphqlRequest<
    { input: { category: string; settings: string } },
    { updatePlatformSettings: { success: boolean } }
  >(
    UPDATE_PLATFORM_SETTINGS_MUTATION,
    { input: { category: 'events', settings: JSON.stringify(settings) } },
    opts.token,
    opts.tenantSlug
  );
  return data.updatePlatformSettings.success;
}
