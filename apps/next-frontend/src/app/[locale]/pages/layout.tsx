import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function PagesLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="pages">{children}</ModuleGuard>;
}
