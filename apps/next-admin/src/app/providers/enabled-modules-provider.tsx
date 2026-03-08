import { auth } from '@/auth';
import { fetchEnabledModules } from '@/shared/api/modules';
import { EnabledModulesClientProvider } from '@/shared/lib/enabled-modules-context';

export async function EnabledModulesProvider({
  children
}: {
  children: React.ReactNode;
}) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const enabledModules =
    token && tenantSlug
      ? await fetchEnabledModules({ token, tenantSlug })
      : [];

  return (
    <EnabledModulesClientProvider initialModules={enabledModules}>
      {children}
    </EnabledModulesClientProvider>
  );
}
