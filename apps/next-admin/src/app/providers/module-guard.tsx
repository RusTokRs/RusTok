import { notFound } from 'next/navigation';
import { auth } from '@/auth';
import { fetchEnabledModules } from '@/shared/api/modules';

export async function ModuleGuard({
  slug,
  children,
  fallback
}: {
  slug: string;
  children: React.ReactNode;
  fallback?: React.ReactNode;
}) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  let enabledModules: string[] = [];

  if (token && tenantSlug) {
    try {
      enabledModules = await fetchEnabledModules({ token, tenantSlug });
    } catch (error) {
      console.warn(`Failed to verify module '${slug}' availability.`, error);
    }
  }

  if (!enabledModules.includes(slug)) {
    if (fallback) {
      return <>{fallback}</>;
    }
    notFound();
  }

  return <>{children}</>;
}
