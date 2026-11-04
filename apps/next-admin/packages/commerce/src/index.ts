/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import React from 'react';
import { registerAdminModule } from '@/modules/registry';
import { commerceNavItems } from './nav';
import {
  CommerceMetricsWidget,
  CommerceRecentOrdersWidget
} from './widgets/commerce-dashboard-widgets';

registerAdminModule({
  id: 'commerce',
  name: 'Commerce Management',
  navItems: commerceNavItems,
  components: [
    {
      id: 'commerce:metrics',
      slot: 'dashboard:metric',
      order: 20,
      render: (context) => React.createElement(CommerceMetricsWidget, { context })
    },
    {
      id: 'commerce:recent-orders',
      slot: 'dashboard:section',
      order: 10,
      render: (context) => React.createElement(CommerceRecentOrdersWidget, { context })
    }
  ]
});

export { commerceNavItems } from './nav';
export * from './types';
export * from './api';
export * from './components/ShippingProfilesTemplate';
export * from './components/CartPromotionsTemplate';
export * from './components/OrderChangesTemplate';
export * from './components/ReturnDecisionsTemplate';
export * from './components/orders-table';
