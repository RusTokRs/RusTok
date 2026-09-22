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
  let enabledModules: string[] = [];

  if (token && tenantSlug) {
    try {
      enabledModules = await fetchEnabledModules({ token, tenantSlug });
    } catch (error) {
      console.warn(
        'Failed to load enabled modules for Next admin shell.',
        error
      );
    }
  }

  return (
    <EnabledModulesClientProvider initialModules={enabledModules}>
      {children}
    </EnabledModulesClientProvider>
  );
}
