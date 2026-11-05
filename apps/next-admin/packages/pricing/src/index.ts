import { registerAdminModule } from '@/modules/registry';
import { pricingNavItems } from './nav';

registerAdminModule({
  id: 'pricing',
  name: 'Pricing',
  navItems: pricingNavItems
});

export { pricingNavItems } from './nav';
export { default as PricingPage } from './pages/pricing-page';

