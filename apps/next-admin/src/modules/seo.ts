import { registerAdminModule } from './registry';

registerAdminModule({
  id: 'seo',
  name: 'SEO',
  navItems: [
    {
      title: 'SEO',
      url: '/dashboard/seo',
      shortcut: ['s', 'o'],
      access: { role: 'admin' },
      group: 'modulePlugins'
    }
  ]
});
