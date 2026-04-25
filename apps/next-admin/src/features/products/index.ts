import { registerAdminModule } from '@/modules/registry';
import { productNavItems } from '../../../packages/rustok-product/src';

registerAdminModule({
  id: 'product',
  name: 'Product Catalog',
  navItems: productNavItems
});

export { productNavItems } from '../../../packages/rustok-product/src';
