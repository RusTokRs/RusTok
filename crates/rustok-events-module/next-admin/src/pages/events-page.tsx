import { getEventDeliveryConfiguration, getEventsStatus } from '../api/events';
import { EventsForm } from '../components/events-form';

interface EventsPageProps {
  token: string | null;
  tenantSlug: string | null;
  errorMessage: string;
}

export async function EventsPage({
  token,
  tenantSlug,
  errorMessage
}: EventsPageProps) {
  const opts = { token, tenantSlug };

  let status;
  let configuration;
  try {
    [status, configuration] = await Promise.all([
      getEventsStatus(opts),
      getEventDeliveryConfiguration(opts)
    ]);
  } catch {
    return <p className='text-destructive text-sm'>{errorMessage}</p>;
  }

  return (
    <EventsForm
      status={status}
      configuration={configuration}
      token={token}
      tenantSlug={tenantSlug}
    />
  );
}
