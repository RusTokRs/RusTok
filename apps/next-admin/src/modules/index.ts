// Admin modules register their nav through module-owned package entrypoints.
// Host shell code should not import business UI feature folders directly.
import '../../packages/blog/src';
import '../../packages/cache/src';
import '../../packages/commerce/src';
import '../../packages/email/src';
import '@rustok/events-admin';
import '@rustok/iggy-connector-admin';
import '../../packages/rbac/src';
import '../../packages/rustok-product/src';
import '../../packages/workflow/src';

export type { AdminModule } from './types';
export {
  registerAdminModule,
  getAdminModules,
  getAdminNavItems
} from './registry';
