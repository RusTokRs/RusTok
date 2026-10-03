// Admin modules register their nav through module-owned package entrypoints.
// Host shell code should not import business UI feature folders directly.
import '@rustok/blog-admin';
import '@rustok/cache-admin/register';
import '@rustok/commerce-admin';
import '@rustok/email-admin';
import '@rustok/events-admin/register';
import '@rustok/forum-admin';
import '@rustok/iggy-connector-admin';
import '@rustok/rbac-admin';
import '@rustok/product-admin';
import '@rustok/translation-admin';
import '@rustok/workflow-admin';

export type { AdminModule } from './types';
export {
  registerAdminModule,
  getAdminModules,
  getAdminNavItems
} from './registry';
