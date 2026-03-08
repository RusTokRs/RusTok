import { EnabledModulesProvider } from '@/app/providers/enabled-modules-provider';
import { KBar } from '@/widgets/command-palette';
import { AppSidebar, Header, InfoSidebar } from '@/widgets/app-shell';
import { InfobarProvider } from '@/shared/ui/shadcn/infobar';
import { SidebarInset, SidebarProvider } from '@/shared/ui/shadcn/sidebar';
import type { Metadata } from 'next';
import { cookies } from 'next/headers';

export const metadata: Metadata = {
  title: 'Next Shadcn Dashboard Starter',
  description: 'Basic dashboard with Next.js and Shadcn'
};

export default async function DashboardLayout({
  children
}: {
  children: React.ReactNode;
}) {
  const cookieStore = await cookies();
  const defaultOpen = cookieStore.get('sidebar_state')?.value === 'true';

  return (
    <EnabledModulesProvider>
      <KBar>
        <SidebarProvider defaultOpen={defaultOpen}>
          <InfobarProvider defaultOpen={false}>
            <AppSidebar />
            <SidebarInset>
              <Header />
              {children}
            </SidebarInset>
            <InfoSidebar side='right' />
          </InfobarProvider>
        </SidebarProvider>
      </KBar>
    </EnabledModulesProvider>
  );
}
