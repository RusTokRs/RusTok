import { auth } from '@/auth';
import { fetchEnabledModules } from '@/shared/api/modules';
import { EnabledModulesClientProvider } from '@/shared/lib/enabled-modules-context';
import { ModulePolicyError } from './module-policy-error';

export async function EnabledModulesProvider({
  children
}: {
  children: React.ReactNode;
}) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  if (!token || !tenantSlug) return <ModulePolicyError />;

  let enabledModules: string[];
  try {
    enabledModules = await fetchEnabledModules({ token, tenantSlug });
  } catch (error) {
    // This provider is in the dashboard layout, outside that segment's
    // error boundary. Never supply an empty policy after a failed read.
    console.error('Failed to fetch module availability', error);
    return <ModulePolicyError />;
  }

  return (
    <EnabledModulesClientProvider initialModules={enabledModules}>
      {children}
    </EnabledModulesClientProvider>
  );
}
