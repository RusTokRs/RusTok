import { listRoles, type GqlOpts } from '../api/roles';
import { RolesTable } from '../components/roles-table';

interface RolesPageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function RolesPage({ token, tenantSlug }: RolesPageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  const roles = await listRoles(opts);

  return <RolesTable roles={roles} />;
}
