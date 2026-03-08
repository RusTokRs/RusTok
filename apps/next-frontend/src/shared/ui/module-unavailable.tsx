type ModuleUnavailableProps = {
  slug?: string;
  title?: string;
  description?: string;
};

export function ModuleUnavailable({
  slug,
  title = 'Module unavailable',
  description
}: ModuleUnavailableProps) {
  return (
    <div className="rounded-3xl border border-border bg-card p-6 shadow-sm">
      <h2 className="text-lg font-semibold text-card-foreground">{title}</h2>
      <p className="mt-2 text-sm text-muted-foreground">
        {description ??
          (slug
            ? `The "${slug}" module is disabled for the current tenant.`
            : 'This module is disabled for the current tenant.')}
      </p>
    </div>
  );
}
