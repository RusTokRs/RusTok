import { fetchEnabledModules } from "@/shared/api/modules";
import { EnabledModulesClientProvider } from "@/shared/lib/enabled-modules-context";
import { ModulePolicyError } from "./module-policy-error";

export async function EnabledModulesProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  let enabledModules: string[];
  try {
    enabledModules = await fetchEnabledModules();
  } catch {
    // This provider is in the root layout, outside route error boundaries.
    return <ModulePolicyError />;
  }

  return (
    <EnabledModulesClientProvider initialModules={enabledModules}>
      {children}
    </EnabledModulesClientProvider>
  );
}
