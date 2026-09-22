import { Icons } from '@/shared/ui/icons';

export interface PermissionCheck {
  permission?: string;
  plan?: string;
  feature?: string;
  role?: string;
  requireOrg?: boolean;
}

export type NavGroupKey =
  'overview' | 'management' | 'modulePlugins' | 'account';

export interface NavItem {
  title: string;
  url: string;
  i18nKey?: string;
  group?: NavGroupKey;
  moduleSlug?: string;
  disabled?: boolean;
  external?: boolean;
  shortcut?: [string, string];
  icon?: keyof typeof Icons;
  label?: string;
  description?: string;
  isActive?: boolean;
  items?: NavItem[];
  access?: PermissionCheck;
}
