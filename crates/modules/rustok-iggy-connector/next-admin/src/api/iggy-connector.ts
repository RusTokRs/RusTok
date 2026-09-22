import { graphqlRequest } from "@/lib/graphql";

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

export interface IggyConnectorConfiguration {
  activeMode: string;
  desiredMode: string;
  bundledAvailable: boolean;
  externalAddresses: string[];
  externalUsername: string;
  passwordResolver: string;
  passwordKey: string;
  passwordConfigured: boolean;
  tlsEnabled: boolean;
  tlsDomain: string | null;
  configured: boolean;
  configurationError: string | null;
  restartRequired: boolean;
}

export interface IggyConnectorInput {
  mode: string;
  externalAddresses: string[];
  externalUsername: string;
  passwordResolver: string;
  passwordKey: string;
  tlsEnabled: boolean;
  tlsDomain: string | null;
}

const CONFIGURATION_QUERY = `
query IggyConnectorConfiguration {
  iggyConnectorConfiguration {
    activeMode desiredMode bundledAvailable externalAddresses externalUsername
    passwordResolver passwordKey passwordConfigured tlsEnabled tlsDomain
    configured configurationError restartRequired
  }
}`;

const UPDATE_MUTATION = `
mutation UpdateIggyConnectorConfiguration($input: UpdateIggyConnectorConfigurationInput!) {
  updateIggyConnectorConfiguration(input: $input) {
    desiredMode configured restartRequired
  }
}`;

export async function getIggyConnectorConfiguration(
  opts: GqlOpts = {},
): Promise<IggyConnectorConfiguration> {
  const data = await graphqlRequest<
    Record<string, never>,
    { iggyConnectorConfiguration: IggyConnectorConfiguration }
  >(CONFIGURATION_QUERY, {}, opts.token, opts.tenantSlug);
  return data.iggyConnectorConfiguration;
}

export async function saveIggyConnectorConfiguration(
  input: IggyConnectorInput,
  opts: GqlOpts = {},
): Promise<
  Pick<
    IggyConnectorConfiguration,
    "desiredMode" | "configured" | "restartRequired"
  >
> {
  const data = await graphqlRequest<
    { input: IggyConnectorInput },
    {
      updateIggyConnectorConfiguration: Pick<
        IggyConnectorConfiguration,
        "desiredMode" | "configured" | "restartRequired"
      >;
    }
  >(UPDATE_MUTATION, { input }, opts.token, opts.tenantSlug);
  return data.updateIggyConnectorConfiguration;
}
