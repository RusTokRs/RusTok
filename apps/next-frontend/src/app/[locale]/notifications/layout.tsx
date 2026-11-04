import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function NotificationsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="notifications">{children}</ModuleGuard>;
}
