import { registerAdminModule } from '@/modules/registry';

import { cacheNavItems } from './nav';

registerAdminModule({
  id: 'cache',
  name: 'Cache',
  navItems: cacheNavItems
});
