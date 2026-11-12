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
        title: 'Orders',
        url: '/dashboard/commerce/orders',
        i18nKey: 'orders'
      },
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
