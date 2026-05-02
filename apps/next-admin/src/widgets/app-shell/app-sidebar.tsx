'use client';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from '@/shared/ui/shadcn/collapsible';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/shared/ui/shadcn/dropdown-menu';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarRail
} from '@/shared/ui/shadcn/sidebar';
import { UserAvatarProfile } from './user-avatar-profile';
import { navItems } from '@/shared/config/nav-config';
import { useMediaQuery } from '@/shared/hooks/use-media-query';
import { useFilteredNavItems } from '@/shared/hooks/use-nav';
import {
  IconChevronRight,
  IconChevronsDown,
  IconLogout,
  IconUserCircle
} from '@tabler/icons-react';
import { useSession, signOut } from 'next-auth/react';
import { useTranslations } from 'next-intl';
import Link from 'next/link';
import { usePathname, useRouter } from 'next/navigation';
import * as React from 'react';
import { Icons } from '@/shared/ui/icons';
import { OrgSwitcher } from './org-switcher';
import type { NavGroupKey, NavItem } from '@/shared/types';

type SidebarNavGroup = {
  key: NavGroupKey;
  items: NavItem[];
};

const NAV_GROUP_ORDER: NavGroupKey[] = [
  'overview',
  'management',
  'modulePlugins',
  'account'
];

function isHrefActive(pathname: string, href: string) {
  if (!href || href === '#') return false;
  return pathname === href || pathname.startsWith(`${href}/`);
}

function isNavItemActive(pathname: string, item: NavItem): boolean {
  return (
    isHrefActive(pathname, item.url) ||
    item.items?.some((subItem) => isNavItemActive(pathname, subItem)) ||
    false
  );
}

function groupSidebarItems(items: NavItem[]): SidebarNavGroup[] {
  const grouped = new Map<NavGroupKey, NavItem[]>();

  items.forEach((item) => {
    const groupKey: NavGroupKey =
      item.group ?? (item.moduleSlug ? 'modulePlugins' : 'management');
    grouped.set(groupKey, [...(grouped.get(groupKey) ?? []), item]);
  });

  return NAV_GROUP_ORDER.map((key) => ({
    key,
    items: grouped.get(key) ?? []
  })).filter((group) => group.items.length > 0);
}

function getNavLabel(t: ReturnType<typeof useTranslations>, item: NavItem) {
  return item.i18nKey ? t(item.i18nKey) : item.title;
}

export default function AppSidebar() {
  const pathname = usePathname();
  const tNav = useTranslations('app.nav');
  const tMenu = useTranslations('app.menu');
  const { isOpen } = useMediaQuery();
  const router = useRouter();
  const { data: session } = useSession();
  const filteredItems = useFilteredNavItems(navItems);
  const navGroups = React.useMemo(
    () => groupSidebarItems(filteredItems),
    [filteredItems]
  );

  React.useEffect(() => {}, [isOpen]);

  const handleLogout = () => {
    signOut({ callbackUrl: '/auth/sign-in' });
  };

  // Adapt the session shape to the sidebar avatar component.
  const avatarUser = session?.user
    ? {
        email: session.user.email ?? '',
        name: session.user.name ?? null,
        role: session.user.role
      }
    : null;

  return (
    <Sidebar collapsible='icon'>
      <SidebarHeader>
        <OrgSwitcher />
      </SidebarHeader>
      <SidebarContent className='overflow-x-hidden'>
        {navGroups.map((group) => (
          <SidebarGroup key={group.key}>
            <SidebarGroupLabel>{tNav(`group.${group.key}`)}</SidebarGroupLabel>
            <SidebarMenu>
              {group.items.map((item) => {
                const Icon = item.icon ? Icons[item.icon] : Icons.logo;
                const itemActive = isNavItemActive(pathname, item);
                const itemLabel = getNavLabel(tNav, item);
                return item?.items && item?.items?.length > 0 ? (
                  <Collapsible
                    key={item.title}
                    asChild
                    defaultOpen={item.isActive || itemActive}
                    className='group/collapsible'
                  >
                    <SidebarMenuItem>
                      <CollapsibleTrigger asChild>
                        <SidebarMenuButton
                          tooltip={itemLabel}
                          isActive={itemActive}
                        >
                          {item.icon && <Icon />}
                          <span>{itemLabel}</span>
                          <IconChevronRight className='ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90' />
                        </SidebarMenuButton>
                      </CollapsibleTrigger>
                      <CollapsibleContent>
                        <SidebarMenuSub>
                          {item.items?.map((subItem) => (
                            <SidebarMenuSubItem key={subItem.title}>
                              <SidebarMenuSubButton
                                asChild
                                isActive={isNavItemActive(pathname, subItem)}
                              >
                                <Link href={subItem.url}>
                                  <span>{getNavLabel(tNav, subItem)}</span>
                                </Link>
                              </SidebarMenuSubButton>
                            </SidebarMenuSubItem>
                          ))}
                        </SidebarMenuSub>
                      </CollapsibleContent>
                    </SidebarMenuItem>
                  </Collapsible>
                ) : (
                  <SidebarMenuItem key={item.title}>
                    <SidebarMenuButton
                      asChild
                      tooltip={itemLabel}
                      isActive={itemActive}
                    >
                      <Link href={item.url}>
                        <Icon />
                        <span>{itemLabel}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroup>
        ))}
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <SidebarMenuButton
                  size='lg'
                  className='data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground'
                >
                  {avatarUser && (
                    <UserAvatarProfile
                      className='h-8 w-8 rounded-lg'
                      showInfo
                      user={avatarUser}
                    />
                  )}
                  <IconChevronsDown className='ml-auto size-4' />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent
                className='w-(--radix-dropdown-menu-trigger-width) min-w-56 rounded-lg'
                side='bottom'
                align='end'
                sideOffset={4}
              >
                <DropdownMenuLabel className='p-0 font-normal'>
                  <div className='px-1 py-1.5'>
                    {avatarUser && (
                      <UserAvatarProfile
                        className='h-8 w-8 rounded-lg'
                        showInfo
                        user={avatarUser}
                      />
                    )}
                  </div>
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuItem
                    onClick={() => router.push('/dashboard/profile')}
                  >
                    <IconUserCircle className='mr-2 h-4 w-4' />
                    {tMenu('profile')}
                  </DropdownMenuItem>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuItem onClick={handleLogout}>
                  <IconLogout className='mr-2 h-4 w-4' />
                  {tMenu('signOut')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
