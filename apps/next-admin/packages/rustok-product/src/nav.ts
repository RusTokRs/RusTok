/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import type { NavItem } from '@/types';

export const productNavItems: NavItem[] = [
  {
    title: 'Catalog',
    url: '#',
    i18nKey: 'catalog',
    group: 'modulePlugins',
    icon: 'product',
    isActive: false,
    items: [
      {
        title: 'Products',
        url: '/dashboard/product',
        i18nKey: 'products',
        shortcut: ['p', 'l']
      },
      {
        title: 'Categories',
        url: '/dashboard/product/categories',
        i18nKey: 'categories'
      },
      {
        title: 'Attributes',
        url: '/dashboard/product/attributes',
        i18nKey: 'attributes'
      },
      {
        title: 'Bundles',
        url: '/dashboard/product/bundles',
        i18nKey: 'bundles'
      }
    ],
    access: { role: 'manager' }
  }
];
