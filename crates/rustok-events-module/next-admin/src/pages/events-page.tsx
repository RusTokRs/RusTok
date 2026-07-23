import { getTranslations } from 'next-intl/server';
import { getEventDeliveryConfiguration, getEventsStatus } from '../api/events';
import { EventsForm } from '../components/events-form';

interface EventsPageProps {
  token: string | null;
  tenantSlug: string | null;
}

export async function EventsPage({ token, tenantSlug }: EventsPageProps) {
  const t = await getTranslations('events');
  const opts = { token, tenantSlug };

  let status;
  let configuration;
  try {
    [status, configuration] = await Promise.all([
      getEventsStatus(opts),
      getEventDeliveryConfiguration(opts)
    ]);
  } catch {
    return <p className='text-destructive text-sm'>{t('error')}</p>;
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
