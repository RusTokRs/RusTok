import type { ReactNode } from "react";
import { ModuleGuard } from "@/app/providers/module-guard";

export default function SearchLayout({
  children,
}: {
  children: ReactNode;
}) {
  return <ModuleGuard slug="search">{children}</ModuleGuard>;
}
