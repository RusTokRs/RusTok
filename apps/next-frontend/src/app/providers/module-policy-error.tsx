"use client";

import { useTranslations } from "@rustok/next-fluent/client";
import { useRouter } from "next/navigation";

/** An unreadable policy is not an authoritative disabled-module decision. */
export function ModulePolicyError() {
  const t = useTranslations("ModulePolicy");
  const router = useRouter();

  return (
    <main className="flex min-h-screen items-center justify-center px-4 py-16">
      <div
        role="alert"
        className="max-w-md space-y-4 rounded-xl border border-border bg-card p-6 text-card-foreground"
      >
        <h1 className="text-xl font-semibold">{t("title")}</h1>
        <p className="text-sm text-muted-foreground">{t("description")}</p>
        <button
          type="button"
          onClick={() => router.refresh()}
          className="rounded-md border border-border px-4 py-2 text-sm"
        >
          {t("retry")}
        </button>
      </div>
    </main>
  );
}
