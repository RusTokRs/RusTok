import { registerAdminModule } from '@/modules/registry';
import { workflowNavItems } from './nav';

registerAdminModule({
  id: 'workflow',
  name: 'Workflows',
  navItems: workflowNavItems
});

export { workflowNavItems } from './nav';
export { default as WorkflowsPage } from './pages/workflows-page';
export { default as WorkflowDetailPage } from './pages/workflow-detail-page';
export { default as WorkflowFormPage } from './pages/workflow-form-page';
export { WorkflowStepEditor } from './components/workflow-step-editor';
export { ExecutionHistory } from './components/execution-history';
export { TemplateGallery } from './components/template-gallery';
export { VersionHistory } from './components/version-history';
export * from './api/workflows';
