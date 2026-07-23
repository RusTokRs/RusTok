import { getIggyConnectorConfiguration } from "../api/iggy-connector";
import { IggyConnectorForm } from "../components/iggy-connector-form";

interface IggyConnectorPageProps {
  token: string | null;
  tenantSlug: string | null;
}

export async function IggyConnectorPage({
  token,
  tenantSlug,
}: IggyConnectorPageProps) {
  try {
    const configuration = await getIggyConnectorConfiguration({
      token,
      tenantSlug,
    });
    return (
      <IggyConnectorForm
        configuration={configuration}
        token={token}
        tenantSlug={tenantSlug}
      />
    );
  } catch {
    return (
      <p className="text-destructive text-sm">
        Iggy connector configuration could not be loaded.
      </p>
    );
  }
}
