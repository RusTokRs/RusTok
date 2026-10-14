import { registerAdminModule } from '@/modules/registry';
import { formsNavItems } from './nav';

registerAdminModule({
  id: 'forms',
  name: 'Forms',
  navItems: formsNavItems
});

export { formsNavItems } from './nav';
export { default as FormsPage } from './pages/forms-page';
export * from './api/forms';
