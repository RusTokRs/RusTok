import { ModuleGuard } from '@/app/providers/module-guard';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

export default function McpLayout({ children }: { children: React.ReactNode }) {
  return (
    <ModuleGuard
      slug='mcp'
      fallback={
        <ModuleUnavailable
          title='MCP module is disabled'
          description='Enable the MCP module on the modules page to manage Model Context Protocol clients.'
        />
      }
    >
      {children}
    </ModuleGuard>
  );
}
