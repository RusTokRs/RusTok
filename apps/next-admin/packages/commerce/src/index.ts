import type { NavItem } from '../../../src/shared/types';
import { registerAdminModule } from '../../../src/modules/registry';

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

// Re-export all submodules
export * from './types';
export * from './api';
export * from './components/ShippingProfilesTemplate';
export * from './components/CartPromotionsTemplate';
export * from './components/OrderChangesTemplate';
