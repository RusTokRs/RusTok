import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

export interface EventsStatus {
  configuredProfile: string;
  iggyMode: string;
  relayIntervalMs: number;
  dlqEnabled: boolean;
  maxAttempts: number;
  pendingEvents: number;
  dlqEvents: number;
  availableTransports: string[];
}

export interface EventDeliveryConfiguration {
  activeProfile: string;
  desiredProfile: string;
  iggyMode: string;
  iggyConfigured: boolean;
  restartRequired: boolean;
}

const EVENTS_STATUS_QUERY = `
query EventsStatus {
  eventsStatus {
    configuredProfile iggyMode relayIntervalMs dlqEnabled maxAttempts
    pendingEvents dlqEvents availableTransports
  }
}`;

const EVENT_DELIVERY_CONFIGURATION_QUERY = `
query EventDeliveryConfiguration {
  eventDeliveryConfiguration {
    activeProfile desiredProfile iggyMode iggyConfigured restartRequired
  }
}`;

const UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION = `
mutation UpdateEventDeliveryConfiguration($input: UpdateEventDeliveryConfigurationInput!) {
  updateEventDeliveryConfiguration(input: $input) {
    desiredProfile restartRequired
  }
}`;

export async function getEventsStatus(
  opts: GqlOpts = {}
): Promise<EventsStatus> {
  const data = await graphqlRequest<Record<string, never>, { eventsStatus: EventsStatus }>(
    EVENTS_STATUS_QUERY,
    {},
    opts.token,
    opts.tenantSlug
  );
  return data.eventsStatus;
}

export async function getEventDeliveryConfiguration(
  opts: GqlOpts = {}
): Promise<EventDeliveryConfiguration> {
  const data = await graphqlRequest<
    Record<string, never>,
    { eventDeliveryConfiguration: EventDeliveryConfiguration }
  >(EVENT_DELIVERY_CONFIGURATION_QUERY, {}, opts.token, opts.tenantSlug);
  return data.eventDeliveryConfiguration;
}

export async function saveEventDeliveryProfile(
  profile: string,
  opts: GqlOpts = {}
): Promise<EventDeliveryConfiguration> {
  const data = await graphqlRequest<
    { input: { profile: string } },
    { updateEventDeliveryConfiguration: Pick<EventDeliveryConfiguration, 'desiredProfile' | 'restartRequired'> }
  >(
    UPDATE_EVENT_DELIVERY_CONFIGURATION_MUTATION,
    { input: { profile } },
    opts.token,
    opts.tenantSlug
  );
  return {
    desiredProfile: data.updateEventDeliveryConfiguration.desiredProfile,
    restartRequired: data.updateEventDeliveryConfiguration.restartRequired,
    activeProfile: '',
    iggyMode: '',
    iggyConfigured: true
  };
}
