//! Browser composition only: Cart owns the controller and UI, Product owns selection and quotes.

use std::cell::{Cell, RefCell};

use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use crate::enabled_module_events::{MODULES_CHANGED, cart_available};

thread_local! {
    static UNMOUNT: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    // A disposed owner can still have network requests in flight. Do not create a second owner
    // in this document until a same-realm operation barrier exists; a full reload restores it.
    static STOPPED: Cell<bool> = const { Cell::new(false) };
}

fn error(message: &'static str) -> JsValue {
    JsValue::from_str(message)
}

fn unique_element(
    document: &web_sys::Document,
    selector: &str,
) -> Result<Option<web_sys::HtmlElement>, JsValue> {
    let nodes = document.query_selector_all(selector)?;
    if nodes.length() > 1 {
        return Err(error("Duplicate storefront client mount point"));
    }
    nodes
        .item(0)
        .map(|node| {
            node.dyn_into::<web_sys::HtmlElement>()
                .map_err(|_| error("Invalid storefront client mount point"))
        })
        .transpose()
}

fn matches_mount_point(
    document: &web_sys::Document,
    selector: &str,
    expected: Option<&web_sys::HtmlElement>,
) -> bool {
    match (expected, unique_element(document, selector)) {
        (None, Ok(None)) => true,
        (Some(expected), Ok(Some(actual))) => {
            expected.is_connected()
                && actual.is_same_node(Some(expected.unchecked_ref::<web_sys::Node>()))
        }
        _ => false,
    }
}

fn module_projection() -> Option<String> {
    let document = web_sys::window()?.document()?;
    let nodes = document
        .query_selector_all("meta[name='rustok-enabled-modules']")
        .ok()?;
    if nodes.length() != 1 {
        return None;
    }
    let element = nodes.item(0)?.dyn_into::<web_sys::Element>().ok()?;
    let content = element.get_attribute("content")?;
    (content.len() <= 16 * 1024).then_some(content)
}

struct ModuleListener {
    window: web_sys::Window,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}
impl Drop for ModuleListener {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            MODULES_CHANGED,
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

fn install_module_listener(
    capability: rustok_cart_storefront::CartCapability,
    expected: StoredValue<String>,
) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
        let announced = event
            .dyn_ref::<web_sys::CustomEvent>()
            .and_then(|event| event.detail().as_string());
        let actual = module_projection();
        let coherent = announced
            .as_ref()
            .zip(actual.as_ref())
            .is_some_and(|(a, b)| a == b);
        let value = if coherent {
            actual.unwrap_or_default()
        } else {
            String::new()
        };
        let enabled = coherent && cart_available(&value);
        expected.set_value(value);
        // Every host replacement, even an equal value, advances the synchronous ABA fence.
        capability.replace(enabled);
    }) as Box<dyn FnMut(web_sys::Event)>);
    if window
        .add_event_listener_with_callback(MODULES_CHANGED, callback.as_ref().unchecked_ref())
        .is_err()
    {
        return false;
    }
    let listener = StoredValue::new_local(Some(ModuleListener { window, callback }));
    on_cleanup(move || listener.set_value(None));
    true
}

/// The bootstrap installs this synchronous fence before loading WASM. A missing, replaced,
/// or invalidated guard must never be interpreted as permission to use an old route.
fn navigation_call(method: &str, argument: Option<&str>) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(guard) = js_sys::Reflect::get(
        window.as_ref(),
        &JsValue::from_str("__rustokStorefrontNavigation"),
    ) else {
        return false;
    };
    let Ok(function) = js_sys::Reflect::get(&guard, &JsValue::from_str(method))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    else {
        return false;
    };
    let result = match argument {
        Some(argument) => function.call1(&guard, &JsValue::from_str(argument)),
        None => function.call0(&guard),
    };
    result.ok().and_then(|value| value.as_bool()) == Some(true)
}

/// URL persistence preserves unrelated selectors and never crosses a changed route.
fn persist_reference(path: &str, cart_id: Option<String>) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let location = window.location();
    let (Ok(current_path), Ok(search), Ok(hash)) =
        (location.pathname(), location.search(), location.hash())
    else {
        return false;
    };
    if current_path != path
        || rustok_ui_core::decode_single_value_ui_query(search.strip_prefix('?').unwrap_or(&search))
            .is_err()
    {
        return false;
    }
    let target = rustok_ui_core::apply_ui_query_pairs(
        &format!("{current_path}{search}"),
        &[("cart_id", cart_id)],
    );
    navigation_call("replace", Some(&format!("{target}{hash}")))
}

/// Product selection changes share the very same navigation fence as Cart reference writes.
/// Return the actual URL so Product links do not reuse a stale Cart/host query snapshot.
fn persist_selection(path: &str, href: &str) -> Option<String> {
    let location = web_sys::window()?.location();
    let search = location.search().ok()?;
    let hash = location.hash().ok()?;
    let target = crate::cart_buyer_route::selection_target(path, href, &search)?;
    let target = format!("{target}{hash}");
    navigation_call("replace", Some(&target)).then_some(target)
}

fn mark_product_error(root: &web_sys::HtmlElement) {
    let _ = root.set_attribute("data-product-buyer-client-error", "true");
    let Some(document) = root.owner_document() else {
        return;
    };
    let Ok(notice) = document.create_element("p") else {
        return;
    };
    let _ = notice.set_attribute("role", "alert");
    notice.set_text_content(Some(
        &root
            .get_attribute("data-product-buyer-client-error-label")
            .unwrap_or_else(|| "Product interaction is unavailable.".into()),
    ));
    let _ = root.append_child(&notice);
}

pub(crate) fn start() -> Result<(), JsValue> {
    if UNMOUNT.with(|mount| mount.borrow().is_some()) {
        return Ok(());
    }
    if STOPPED.with(Cell::get) || !navigation_call("current", None) {
        return Err(error("Storefront navigation requires a fresh page"));
    }
    let window = web_sys::window().ok_or_else(|| error("Storefront window is unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| error("Storefront document is unavailable"))?;
    let host = unique_element(&document, "[data-rustok-cart-host='true']")?
        .ok_or_else(|| error("Storefront Cart host is unavailable"))?;
    let header = unique_element(&document, "[data-rustok-cart-island='true']")?;
    let product = unique_element(&document, "[data-product-buyer-island='true']")?;
    let tenant = host
        .get_attribute("data-tenant")
        .filter(|value| {
            !value.is_empty()
                && value.trim() == value
                && value.len() <= 255
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| error("Storefront tenant is unavailable"))?;
    let locale = host
        .get_attribute("data-locale")
        .filter(|value| !value.is_empty());
    let path = window.location().pathname()?;
    let query = window.location().search()?;
    let query =
        rustok_ui_core::decode_single_value_ui_query(query.strip_prefix('?').unwrap_or(&query))
            .map_err(|_| error("Storefront route is ambiguous"))?;
    let cart_id = query.get("cart_id").cloned();
    let projection = module_projection().unwrap_or_default();
    let root = header.clone().unwrap_or_else(|| host.clone());
    let show_header = header.is_some();
    root.set_inner_html("");

    let handle = mount_to(root, move || {
        let expected = StoredValue::new(projection.clone());
        let expected_tenant = StoredValue::new(tenant.clone());
        let expected_locale = StoredValue::new(locale.clone());
        let mounts = StoredValue::new_local((host, header, product));
        let verify = Callback::new(move |_| {
            let markers_current = mounts.try_with_value(|(host, header, product)| {
                let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                    return false;
                };
                matches_mount_point(&document, "[data-rustok-cart-host='true']", Some(host))
                    && matches_mount_point(
                        &document,
                        "[data-rustok-cart-island='true']",
                        header.as_ref(),
                    )
                    && matches_mount_point(
                        &document,
                        "[data-product-buyer-island='true']",
                        product.as_ref(),
                    )
                    && host.get_attribute("data-tenant") == expected_tenant.try_get_value()
                    && host.get_attribute("data-locale")
                        == expected_locale.try_get_value().flatten()
            }) == Some(true);
            markers_current
                && navigation_call("current", None)
                && module_projection() == expected.try_get_value()
        });
        let capability = rustok_cart_storefront::CartCapability::with_host_verifier(
            cart_available(&projection),
            verify,
        );
        if !install_module_listener(capability, expected) {
            capability.fail_closed();
        }
        let path = StoredValue::new(path);
        let persist = Callback::new(move |id| {
            path.try_get_value()
                .is_some_and(|path| persist_reference(&path, id))
        });
        let persist_selection = Callback::new(move |href: String| {
            path.try_get_value()
                .and_then(|path| persist_selection(&path, &href))
        });
        let expected_query = StoredValue::new(query);
        let product_cleanup = StoredValue::new_local(None::<Box<dyn FnOnce()>>);
        on_cleanup(move || {
            if let Some(cleanup) = product_cleanup
                .try_update_value(|value| value.take())
                .flatten()
            {
                cleanup();
            }
        });
        let buyer_tenant = StoredValue::new(tenant.clone());
        let on_ready = Callback::new(move |cart| {
            if let Some(root) = mounts
                .try_with_value(|(_, _, product)| product.clone())
                .flatten()
            {
                let (Some(tenant), Some(query)) =
                    (buyer_tenant.try_get_value(), expected_query.try_get_value())
                else {
                    return;
                };
                match rustok_product_storefront::mount_product_buyer(
                    root.clone(),
                    &tenant,
                    &query,
                    cart,
                ) {
                    Ok(cleanup) => product_cleanup.set_value(Some(cleanup)),
                    Err(_) => mark_product_error(&root),
                }
            }
        });
        view! {
            <leptos_auth::AuthProvider tenant_slug=tenant.clone()>
                <rustok_cart_storefront::CartClient cart_id locale tenant_slug=tenant
                    capability persist_reference=persist persist_selection on_ready show_header />
            </leptos_auth::AuthProvider>
        }
    });
    UNMOUNT.with(|mount| *mount.borrow_mut() = Some(Box::new(move || drop(handle))));
    Ok(())
}

/// A client-side host replacing the whole shell must dispose the old owner before remounting.
pub(crate) fn stop() {
    if let Some(unmount) = UNMOUNT.with(|mount| mount.borrow_mut().take()) {
        STOPPED.with(|stopped| stopped.set(true));
        unmount();
    }
}
