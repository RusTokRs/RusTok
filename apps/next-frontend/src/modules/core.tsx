import { registerStorefrontModule } from "./registry";

function ModuleSpotlight() {
  return (
    <div className="rounded-3xl border border-border bg-secondary p-6 shadow-sm">
      <h3 className="text-lg font-semibold text-foreground">
        Modular storefront sections
      </h3>
      <p className="mt-2 text-sm text-muted-foreground">
        Extend the home page by registering React components from optional
        packages.
      </p>
      <div className="mt-4 flex flex-wrap gap-2 text-xs">
        <span className="rounded-full bg-primary/10 px-3 py-1 font-semibold text-primary">
          Next.js
        </span>
        <span className="rounded-full bg-emerald-100 px-3 py-1 font-semibold text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
          Registry
        </span>
        <span className="rounded-full bg-muted px-3 py-1 font-semibold text-muted-foreground">
          Packages
        </span>
      </div>
    </div>
  );
}

registerStorefrontModule({
  id: "core-module-spotlight",
  slot: "home:afterHero",
  order: 10,
  render: () => <ModuleSpotlight />,
});
