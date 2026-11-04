import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function ForumLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="forum">{children}</ModuleGuard>;
}
