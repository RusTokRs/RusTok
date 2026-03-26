import { registerAdminModule } from '@/modules/registry';
import { emailNavItems } from './nav';

registerAdminModule({
  id: 'email',
  name: 'Email Settings',
  navItems: emailNavItems
});

export { emailNavItems } from './nav';
export { default as EmailSettingsPage } from './pages/email-settings-page';
export { EmailSettingsForm } from './components/email-settings-form';
export * from './api/email';
