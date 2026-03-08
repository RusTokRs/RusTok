import { CheckCircle2, Rocket, Sparkles } from "lucide-react";
import { getTranslations } from "next-intl/server";

import { getModulesForSlot } from "@/modules";
import { fetchEnabledModules } from "@/shared/api/modules";
import { Button } from "@/shared/ui/base/button";

export default async function StorefrontHome() {
  const t = await getTranslations("Storefront");
  const features = t.raw("features") as string[];
  const chips = t.raw("chips") as string[];
  const enabledModules = await fetchEnabledModules();
  const moduleSections = getModulesForSlot("home:afterHero", enabledModules);

  return (
    <main className="min-h-screen bg-background">
      <section className="mx-auto max-w-6xl px-6 py-16">
        <div className="grid gap-10 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
          <div>
            <p className="text-sm font-semibold uppercase tracking-wide text-primary">
              {t("eyebrow")}
            </p>
            <h1 className="mt-3 text-4xl font-semibold text-foreground sm:text-5xl">
              {t("title")}
            </h1>
            <p className="mt-4 text-lg text-muted-foreground">
              {t("subtitle")}
            </p>
            <div className="mt-6 flex flex-wrap gap-3">
              <Button size="lg">{t("primaryCta")}</Button>
              <Button size="lg" variant="outline">
                {t("secondaryCta")}
              </Button>
            </div>
            <ul className="mt-8 space-y-3 text-sm text-muted-foreground">
              {features.map((feature) => (
                <li key={feature} className="flex items-center gap-2">
                  <CheckCircle2 className="h-4 w-4 text-emerald-500" />
                  {feature}
                </li>
              ))}
            </ul>
          </div>
          <div className="rounded-3xl border border-border bg-secondary p-6 shadow-sm">
            <div className="space-y-6">
              <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10 text-primary">
                    <Rocket className="h-5 w-5" />
                  </div>
                  <div>
                    <p className="text-sm font-semibold text-card-foreground">
                      {t("cards.launchTitle")}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {t("cards.launchDescription")}
                    </p>
                  </div>
                </div>
              </div>
              <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-amber-100 text-amber-600 dark:bg-amber-900/30 dark:text-amber-400">
                    <Sparkles className="h-5 w-5" />
                  </div>
                  <div>
                    <p className="text-sm font-semibold text-card-foreground">
                      {t("cards.flexTitle")}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {t("cards.flexDescription")}
                    </p>
                  </div>
                </div>
              </div>
              <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
                <p className="text-sm font-semibold text-card-foreground">
                  {t("cards.stackTitle")}
                </p>
                <div className="mt-3 flex flex-wrap gap-2 text-xs">
                  {chips.map((chip) => (
                    <span
                      key={chip}
                      className="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-foreground"
                    >
                      {chip}
                    </span>
                  ))}
                </div>
              </div>
              <div className="rounded-lg border border-primary/20 bg-primary/5 px-4 py-3 text-sm text-foreground">
                <h3 className="font-semibold">{t("alertTitle")}</h3>
                <p className="mt-1 text-muted-foreground">
                  {t("alertDescription")}
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>
      {moduleSections.map((module) => (
        <section key={module.id} className="mx-auto max-w-6xl px-6 pb-12">
          {module.render()}
        </section>
      ))}
    </main>
  );
}
