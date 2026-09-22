import type { NavItem } from '@/types';

export interface AdminModule {
  id: string;
  name: string;
  navItems: NavItem[];
}
