import { getTranslations } from 'next-intl/server';
import { getEventsStatus, getEventsSettings } from '../api/events';
import { EventsForm } from '../components/events-form';

interface EventsPageProps {
  token: string | null;
  tenantSlug: string | null;
}

export async function EventsPage({ token, tenantSlug }: EventsPageProps) {
  const t = await getTranslations('events');
  const opts = { token, tenantSlug };

  let status;
  let initialSettings = null;
  try {
    [status, initialSettings] = await Promise.all([
      getEventsStatus(opts),
      getEventsSettings(opts)
    ]);
  } catch {
    return <p className='text-destructive text-sm'>{t('error')}</p>;
  }

  return (
    <EventsForm
      status={status}
      initialSettings={initialSettings}
      token={token}
      tenantSlug={tenantSlug}
    />
  );
}
