import 'server-only';
import type { Tenant } from '@/entities/tenant';
import { graphqlRequest } from './graphql';
const QUERY='query CurrentTenant { currentTenant { id name slug } }';
interface ResponseShape {currentTenant:Tenant}
export async function fetchCurrentTenant(token:string,tenantSlug:string):Promise<Tenant|null> {
  try { return (await graphqlRequest<undefined,ResponseShape>(QUERY,undefined,token,tenantSlug)).currentTenant; }
  catch { return null; }
}
