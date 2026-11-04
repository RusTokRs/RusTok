import type React from 'react';
import type { NavItem } from '@/types';

export type AdminSlot =
  | 'dashboard:metric'
  | 'dashboard:section'
  | 'header:actions'
  | 'sidebar:footer';

export type AdminRenderContext = {
  token: string | null;
  tenantSlug: string | null;
  enabledModules: string[];
};

export type AdminComponentRegistration = {
  id: string;
  moduleSlug?: string;
  slot: AdminSlot | string;
  order?: number;
  render: (context: AdminRenderContext) => React.ReactNode;
};

export interface AdminModule {
  id: string;
  name: string;
  navItems: NavItem[];
  components?: AdminComponentRegistration[];
}
