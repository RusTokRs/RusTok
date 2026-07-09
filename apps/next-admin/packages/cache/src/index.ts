import { registerAdminModule } from '@/modules/registry';
import { cacheNavItems } from './nav';

registerAdminModule({
  id: 'cache',
  name: 'Cache',
  navItems: cacheNavItems
});

export { cacheNavItems } from './nav';
export { default as CachePage } from './pages/cache-page';
export { CacheStatus } from './components/cache-status';
export * from './api/cache';
