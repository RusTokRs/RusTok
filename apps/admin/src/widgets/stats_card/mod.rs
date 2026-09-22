use leptos::prelude::*;
use leptos_ui::{Badge, BadgeVariant, Card, CardAction, CardFooter, CardHeader, CardTitle};

#[component]
pub fn stats_card(
    #[prop(into)] title: String,
    #[prop(into)] value: String,
    #[prop(into)] icon: AnyView,
    #[prop(into)] trend: String,
    #[prop(optional, into)] trend_label: Option<String>,
    #[prop(optional, into)] trend_up: Option<bool>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let is_up = trend_up.unwrap_or(true);
    let color_class = if is_up {
        "border-emerald-200 text-emerald-700 dark:border-emerald-900/50 dark:text-emerald-400"
    } else {
        "border-destructive/30 text-destructive"
    };
    let label = trend_label.unwrap_or_default();

    view! {
        <Card class=format!("bg-gradient-to-t from-primary/5 to-card {}", class)>
            <CardHeader class="grid-cols-[1fr_auto]">
                <div class="text-sm text-muted-foreground">{title}</div>
                <CardTitle class="text-2xl font-semibold tabular-nums md:text-3xl">{value}</CardTitle>
                <CardAction class="rounded-lg bg-muted p-3 text-muted-foreground">
                    {icon}
                </CardAction>
            </CardHeader>
            <CardFooter class="flex-col items-start gap-1.5 text-sm">
                <div class="line-clamp-1 flex items-center gap-2 font-medium">
                    <Badge variant=BadgeVariant::Outline class=color_class>
                        {trend}
                    </Badge>
                    {(!label.is_empty()).then(|| view! {
                        <span>{label}</span>
                    })}
                </div>
            </CardFooter>
        </Card>
    }
}
