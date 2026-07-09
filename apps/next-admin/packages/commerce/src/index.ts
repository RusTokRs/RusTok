/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { registerAdminModule } from '@/modules/registry';
import type { NavItem } from '@/types';

export const commerceNavItems: NavItem[] = [
  {
    title: 'Commerce',
    url: '#',
    i18nKey: 'commerce',
    group: 'modulePlugins',
    icon: 'modules',
    isActive: false,
    items: [
      {
        title: 'Shipping Profiles',
        url: '/dashboard/commerce/shipping-profiles',
        i18nKey: 'shippingProfiles'
      },
      {
        title: 'Cart Promotions',
        url: '/dashboard/commerce/promotions',
        i18nKey: 'cartPromotions'
      },
      {
        title: 'Order Changes',
        url: '/dashboard/commerce/order-changes',
        i18nKey: 'orderChanges'
      },
      {
        title: 'Return Decisions',
        url: '/dashboard/commerce/return-decisions',
        i18nKey: 'returnDecisions'
      }
    ],
    access: { role: 'manager' }
  }
];

registerAdminModule({
  id: 'commerce',
  name: 'Commerce Management',
  navItems: commerceNavItems
});

export * from './types';
export * from './api';
export * from './components/ShippingProfilesTemplate';
export * from './components/CartPromotionsTemplate';
export * from './components/OrderChangesTemplate';
export * from './components/ReturnDecisionsTemplate';
