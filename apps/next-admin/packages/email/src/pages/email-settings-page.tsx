import { getEmailSettings, type GqlOpts } from '../api/email';
import { EmailSettingsForm } from '../components/email-settings-form';

interface EmailSettingsPageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function EmailSettingsPage({
  token,
  tenantSlug
}: EmailSettingsPageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  const snapshot = await getEmailSettings(opts);

  return (
    <EmailSettingsForm
      initialSettings={snapshot.settings}
      initialSchemaVersion={snapshot.schemaVersion}
      opts={opts}
    />
  );
}
