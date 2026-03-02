import {
  fetchGraphql,
  type GraphqlRequest,
  type GraphqlResponse,
  GRAPHQL_ENDPOINT,
  AUTH_HEADER,
  TENANT_HEADER,
} from "leptos-graphql/next";

export { GRAPHQL_ENDPOINT, AUTH_HEADER, TENANT_HEADER };
export type { GraphqlRequest, GraphqlResponse };

export type FrontendGraphqlOptions<V> = {
  query: string;
  variables?: V;
  token?: string;
  tenant?: string;
  baseUrl?: string;
};

export async function storefrontGraphql<T, V = Record<string, unknown>>(
  options: FrontendGraphqlOptions<V>,
): Promise<GraphqlResponse<T>> {
  const { query, variables, token, tenant, baseUrl } = options;
  return fetchGraphql<T, V>({
    baseUrl,
    token,
    tenant,
    request: {
      query,
      variables,
    },
  });
}
