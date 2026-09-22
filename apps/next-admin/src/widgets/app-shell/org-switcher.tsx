'use client';
import { GalleryVerticalEnd } from 'lucide-react';
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar
} from '@/shared/ui/shadcn/sidebar';
import { useSession } from 'next-auth/react';

export function OrgSwitcher() {
  const { state } = useSidebar();
  const { data: session } = useSession();

  const tenantSlug = session?.user?.tenantSlug;
  const role = session?.user?.role;

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarMenuButton size='lg' className='cursor-default'>
          <div className='bg-sidebar-primary text-sidebar-primary-foreground flex aspect-square size-8 shrink-0 items-center justify-center rounded-lg'>
            <GalleryVerticalEnd className='size-4' />
          </div>
          <div
            className={`grid flex-1 text-left text-sm leading-tight transition-all duration-200 ease-in-out ${state === 'collapsed' ? 'invisible max-w-0 overflow-hidden opacity-0' : 'visible max-w-full opacity-100'}`}
          >
            <span className='truncate font-medium'>
              {tenantSlug || 'RusTok'}
            </span>
            <span className='text-muted-foreground truncate text-xs'>
              {role || 'Workspace'}
            </span>
          </div>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
