import { registerAdminModule } from '@/modules/registry';
import { pagesNavItems } from './nav';

registerAdminModule({
  id: 'pages',
  name: 'Pages',
  navItems: pagesNavItems
});

export { pagesNavItems } from './nav';
export { default as PagesPage } from './pages/pages-page';
export * from './api/pages';
