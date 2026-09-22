import React from 'react';
import { SidebarTrigger } from '@/shared/ui/shadcn/sidebar';
import { Separator } from '@/shared/ui/shadcn/separator';
import { Breadcrumbs } from '@/shared/ui/breadcrumbs';
import SearchInput from '@/shared/ui/search-input';
import { ThemeModeToggle } from '@/shared/lib/themes/theme-mode-toggle';
import { LanguageSelect } from './language-select';
import { HeaderUserMenu } from './header-user-menu';

export default function Header() {
  return (
    <header className='flex h-16 shrink-0 items-center justify-between gap-2 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12'>
      <div className='flex items-center gap-2 px-4'>
        <SidebarTrigger className='-ml-1' />
        <Separator orientation='vertical' className='mr-2 h-4' />
        <Breadcrumbs />
      </div>

      <div className='flex items-center gap-2 px-4'>
        <div className='hidden md:flex'>
          <SearchInput />
        </div>
        <LanguageSelect />
        <ThemeModeToggle />
        <HeaderUserMenu />
      </div>
    </header>
  );
}
