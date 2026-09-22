import { fetchEnabledModules } from '@/shared/api/modules';
import { EnabledModulesClientProvider } from '@/shared/lib/enabled-modules-context';

export async function EnabledModulesProvider({
  children
}: {
  children: React.ReactNode;
}) {
  const enabledModules = await fetchEnabledModules();

  return (
    <EnabledModulesClientProvider initialModules={enabledModules}>
      {children}
    </EnabledModulesClientProvider>
  );
}
