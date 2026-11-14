//! One mounted Cart owner for the header, drawer, landing bridge and Product buyer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::{UiRouteContext, encode_ui_query_pairs};
use rustok_ui_leptos::{Dialog, DialogContent, DialogTitle};

use super::bridge::DrawerVisibility;
use super::buyer::{BuyerCartController, CartCapability, use_buyer_cart};
use crate::core::{BuyerCartLineAction, BuyerCartPhase, buyer_cart_badge};
use crate::i18n::t;

#[derive(Clone)]
struct Labels {
    cart: String,
    title: String,
    no_reference: String,
    empty: String,
    view_cart: String,
    decrease: String,
    remove: String,
    inspect: String,
    reset: String,
    unknown: String,
    total: String,
}

fn labels(locale: Option<&str>) -> Labels {
    Labels {
        cart: t(locale, "cart-trigger-label", "Cart"),
        title: t(locale, "cart-drawer-title", "Shopping Cart"),
        no_reference: t(
            locale,
            "cart-drawer-noReference",
            "No cart is selected yet.",
        ),
        empty: t(locale, "cart-drawer-empty", "Your cart is empty"),
        view_cart: t(locale, "cart-drawer-viewCart", "View cart"),
        decrease: t(locale, "cart-drawer-decrease", "Decrease quantity"),
        remove: t(locale, "cart-drawer-remove", "Remove item"),
        inspect: t(locale, "cart-drawer-inspect", "Inspect cart"),
        reset: t(locale, "cart-drawer-resetReference", "Reset cart reference"),
        unknown: t(locale, "cart-count-unknown", "Cart count is unknown"),
        total: t(locale, "cart-drawer-total", "Total"),
    }
}

/// The host supplies tenant, locale, policy and URL persistence. Both the landing controls and
/// Product use the resulting controller; no independent drawer resource can bypass its fences.
#[component]
pub fn CartClient(
    cart_id: Option<String>,
    locale: Option<String>,
    tenant_slug: String,
    capability: CartCapability,
    persist_reference: Callback<Option<String>, bool>,
    persist_selection: Callback<String, Option<String>>,
    on_ready: Callback<BuyerCartController>,
    #[prop(default = true)] show_header: bool,
) -> impl IntoView {
    let source = use_context::<rustok_auth_ui::ui::leptos::AuthContext>()
        .map(|auth| auth.scope_source().to_string())
        .unwrap_or_default();
    let controller = use_buyer_cart(
        cart_id,
        locale.clone(),
        tenant_slug,
        capability,
        persist_reference,
        persist_selection,
    );
    let drawer = DrawerVisibility::new();
    if !super::bridge::install(controller, drawer, source) {
        capability.fail_closed();
    }
    on_ready.run(controller);
    let labels = StoredValue::new(labels(locale.as_deref()));
    let locale = StoredValue::new(locale);
    let change_open = Callback::new(move |next: bool| {
        if !next || controller.context_ready() {
            drawer.set(next);
        }
    });
    let visible = Signal::derive(move || drawer.open.get() && controller.context_ready());
    view! {
        <Dialog open=visible on_open_change=change_open>
            {move || capability.get().enabled.then(|| view! {
                <div class="inline-flex items-center">
                    {show_header.then(|| view! { <CartTrigger controller drawer labels floating=false /> })}
                    <CartTrigger controller drawer labels floating=true />
                </div>
            })}
            <DialogContent id="rustok-cart-dialog" aria_labelledby="rustok-cart-dialog-title" class="max-h-[85vh] overflow-y-auto">
                <DialogTitle id="rustok-cart-dialog-title">{labels.get_value().title}</DialogTitle>
                <CartContents controller locale labels />
            </DialogContent>
        </Dialog>
    }
}

#[component]
fn CartTrigger(
    controller: BuyerCartController,
    drawer: DrawerVisibility,
    labels: StoredValue<Labels>,
    floating: bool,
) -> impl IntoView {
    let class = if floating {
        "fixed bottom-6 right-6 z-40 inline-flex h-14 items-center gap-2 rounded-2xl bg-primary px-4 text-primary-foreground shadow-xl disabled:opacity-50"
    } else {
        "inline-flex h-9 items-center gap-2 rounded-xl border border-border px-3 text-xs font-semibold disabled:opacity-50"
    };
    view! {
        <button type="button" class=class disabled=move || !controller.context_ready()
            aria-haspopup="dialog" aria-controls="rustok-cart-dialog"
            aria-expanded=move || drawer.open.get().to_string()
            on:click=move |_| {
                if controller.context_ready() { drawer.toggle(); }
            }>
            <span>{labels.get_value().cart}</span>
            <span aria-label=move || controller.confirmed_item_count().is_none().then(|| labels.get_value().unknown)>
                {move || buyer_cart_badge(controller.confirmed_item_count())}
            </span>
        </button>
    }
}

#[component]
fn CartContents(
    controller: BuyerCartController,
    locale: StoredValue<Option<String>>,
    labels: StoredValue<Labels>,
) -> impl IntoView {
    let base = StoredValue::new(
        UiRouteContext {
            locale: locale.get_value(),
            ..Default::default()
        }
        .module_route_base("cart"),
    );
    view! {
        <div class="mt-4 space-y-4" aria-busy=move || controller.busy.get().to_string()>
            {move || controller.session.get().notice(locale.get_value().as_deref())
                .map(|notice| view! { <p role="status" class="text-sm text-muted-foreground">{notice}</p> })}
            {move || {
                let session = controller.session.get();
                if session.phase == BuyerCartPhase::Empty {
                    Some(view! { <p class="text-sm text-muted-foreground">{labels.get_value().no_reference}</p> })
                } else { None }
            }}
            {move || (controller.confirmed_item_count().is_some())
                .then(|| controller.session.get().cart).flatten().map(|cart| {
                let href = format!("{}?{}", base.get_value(), encode_ui_query_pairs(&[("cart_id", cart.id.as_str())]));
                let total = cart.total_amount.clone();
                let currency = cart.currency_code.clone();
                let empty = cart.line_items.is_empty();
                let lines = cart.line_items.into_iter().map(|item| {
                    let decrement_id = item.id.clone();
                    let remove_id = item.id.clone();
                    view! {
                        <li class="flex items-center justify-between gap-3 rounded-xl border border-border p-3">
                            <div><p class="font-medium">{item.title}</p>
                                <p class="text-sm text-muted-foreground">{item.quantity.to_string()} " × " {item.unit_price} " " {item.currency_code}</p></div>
                            <div class="flex gap-2">
                                <button type="button" class="rounded border border-border px-2 disabled:opacity-50"
                                    disabled=move || !controller.can_write() aria-label=labels.get_value().decrease
                                    on:click=move |_| {
                                        let id = decrement_id.clone();
                                        spawn_local(async move { controller.change_line(id, BuyerCartLineAction::Decrement, locale.get_value()).await; });
                                    }>"−"</button>
                                <button type="button" class="rounded border border-border px-2 disabled:opacity-50"
                                    disabled=move || !controller.can_write() aria-label=labels.get_value().remove
                                    on:click=move |_| {
                                        let id = remove_id.clone();
                                        spawn_local(async move { controller.change_line(id, BuyerCartLineAction::Remove, locale.get_value()).await; });
                                    }>"×"</button>
                            </div>
                        </li>
                    }
                }).collect_view();
                view! {
                    {empty.then(|| view! { <p class="text-sm text-muted-foreground">{labels.get_value().empty}</p> })}
                    <ul class="space-y-2">{lines}</ul>
                    <p class="font-semibold">{labels.get_value().total}": "{total}" "{currency}</p>
                    <a class="inline-flex rounded-xl bg-primary px-4 py-2 text-primary-foreground" href=href>{labels.get_value().view_cart}</a>
                }
            })}
            {move || controller.session.get().can_inspect().then(|| view! {
                <button type="button" class="rounded border border-border px-3 py-2 text-sm disabled:opacity-50"
                    disabled=move || !controller.can_reset() on:click=move |_| controller.inspect()>{labels.get_value().inspect}</button>
            })}
            {move || matches!(controller.session.get().phase, BuyerCartPhase::RestoreFailed | BuyerCartPhase::Unconfirmed).then(|| view! {
                <button type="button" class="rounded border border-border px-3 py-2 text-sm disabled:opacity-50"
                    disabled=move || !controller.can_reset() on:click=move |_| controller.reset_reference()>{labels.get_value().reset}</button>
            })}
        </div>
    }
}
