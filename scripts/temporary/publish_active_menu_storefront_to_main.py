from pathlib import Path
import subprocess

PRODUCT_BRANCH = "origin/agent/pages-active-menu-storefront"
COPY_PATHS = [
    "crates/rustok-pages/storefront/src/model.rs",
    "crates/rustok-pages/storefront/src/transport/graphql_adapter.rs",
    "crates/rustok-pages/storefront/tests/active_menu_transport_contract.rs",
]


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


for path in COPY_PATHS:
    content = subprocess.check_output(
        ["git", "show", f"{PRODUCT_BRANCH}:{path}"], text=True
    )
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)

# Existing generation-aware page cache entries predate active-menu fields.
model_path = Path("crates/rustok-pages/storefront/src/model.rs")
model = model_path.read_text()
model = model.replace(
    '#[serde(rename = "activeHeaderMenu")]',
    '#[serde(default, rename = "activeHeaderMenu")]',
    1,
)
model = model.replace(
    '#[serde(rename = "activeFooterMenu")]',
    '#[serde(default, rename = "activeFooterMenu")]',
    1,
)
model_path.write_text(model)

path = Path("crates/rustok-pages/storefront/src/transport/native_server_adapter.rs")
text = path.read_text()

text = replace_once(
    text,
    '''#[cfg(feature = "ssr")]
use crate::model::{PageBody, PageDetail, PageList, PageListItem, PageTranslation};
''',
    '''#[cfg(feature = "ssr")]
use crate::model::{
    PageBody, PageDetail, PageList, PageListItem, PageTranslation, StorefrontMenu,
    StorefrontMenuItem, StorefrontMenuLocation,
};
''',
    "storefront model imports",
)

text = replace_once(
    text,
    '''        use rustok_pages::{
            ListPagesFilter as RuntimeListPagesFilter, PageBuilderArtifactService, PageService,
            PagesCacheReadRuntime, storefront_pages_cache_key,
        };
''',
    '''        use rustok_pages::{
            ListPagesFilter as RuntimeListPagesFilter, MENU_LOCALE_NOT_FOUND_ERROR_CODE,
            MenuBindingService, MenuLocation, PageBuilderArtifactService, PageService,
            PagesCacheReadRuntime, PagesError, storefront_pages_cache_key,
        };
''',
    "runtime menu imports",
)

marker = '''        let cache_runtime = runtime_ctx.shared_get::<PagesCacheReadRuntime>();
'''
insert = '''        let (active_header_menu, active_footer_menu) =
            if let Some(channel_id) = request_context.as_ref().and_then(|ctx| ctx.channel_id) {
                let binding_service =
                    MenuBindingService::new(runtime_ctx.db_clone(), event_bus.clone());
                let load = |location| {
                    let binding_service = &binding_service;
                    let requested_locale = &requested_locale;
                    async move {
                        match binding_service
                            .get_active(
                                tenant_id,
                                SecurityContext::public_read(),
                                channel_id,
                                location,
                                requested_locale.as_str(),
                            )
                            .await
                        {
                            Ok(menu) => Ok(menu.map(map_storefront_menu)),
                            Err(PagesError::MenuNotFound(_)) => Ok(None),
                            Err(PagesError::Rich(rich))
                                if rich.error_code.as_deref()
                                    == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) =>
                            {
                                Ok(None)
                            }
                            Err(error) => Err(ServerFnError::new(error)),
                        }
                    }
                };
                (
                    load(MenuLocation::Header).await?,
                    load(MenuLocation::Footer).await?,
                )
            } else {
                (None, None)
            };

'''
text = replace_once(text, marker, insert + marker, "active menu load before page cache")

text = replace_once(
    text,
    '''                Ok(Some(cached)) => {
                    tracing::debug!(%tenant_id, "Pages storefront cache hit");
                    return Ok(cached);
                }
''',
    '''                Ok(Some(mut cached)) => {
                    tracing::debug!(%tenant_id, "Pages storefront cache hit");
                    cached.active_header_menu = active_header_menu.clone();
                    cached.active_footer_menu = active_footer_menu.clone();
                    return Ok(cached);
                }
''',
    "cache-hit active menu refresh",
)

text = replace_once(
    text,
    '''            pages: PageList {
                items: items.into_iter().map(map_page_list_item).collect(),
                total,
            },
        };
''',
    '''            pages: PageList {
                items: items.into_iter().map(map_page_list_item).collect(),
                total,
            },
            active_header_menu,
            active_footer_menu,
        };
''',
    "storefront data active menus",
)

text = replace_once(
    text,
    '''        if let (Some(cache_runtime), Some(cache_key)) =
            (cache_runtime.as_ref(), cache_key)
        {
            if let Err(error) = cache_runtime.put_json(cache_key, &data).await {
                tracing::warn!(%error, %tenant_id, "Pages storefront cache fill failed");
            }
        }
''',
    '''        if let (Some(cache_runtime), Some(cache_key)) =
            (cache_runtime.as_ref(), cache_key)
        {
            let mut cached_data = data.clone();
            cached_data.active_header_menu = None;
            cached_data.active_footer_menu = None;
            if let Err(error) = cache_runtime.put_json(cache_key, &cached_data).await {
                tracing::warn!(%error, %tenant_id, "Pages storefront cache fill failed");
            }
        }
''',
    "cache payload excludes active menus",
)

marker = '''#[cfg(feature = "ssr")]
fn published_artifact_page_body(
'''
insert = '''#[cfg(feature = "ssr")]
fn map_storefront_menu(menu: rustok_pages::MenuResponse) -> StorefrontMenu {
    StorefrontMenu {
        id: menu.id.to_string(),
        effective_locale: menu.effective_locale,
        name: menu.name,
        location: match menu.location {
            rustok_pages::MenuLocation::Header => StorefrontMenuLocation::Header,
            rustok_pages::MenuLocation::Footer => StorefrontMenuLocation::Footer,
            rustok_pages::MenuLocation::Sidebar => StorefrontMenuLocation::Sidebar,
            rustok_pages::MenuLocation::Mobile => StorefrontMenuLocation::Mobile,
        },
        items: menu.items.into_iter().map(map_storefront_menu_item).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_storefront_menu_item(item: rustok_pages::MenuItemResponse) -> StorefrontMenuItem {
    StorefrontMenuItem {
        id: item.id.to_string(),
        title: item.title,
        url: item.url,
        icon: item.icon,
        children: item
            .children
            .into_iter()
            .map(map_storefront_menu_item)
            .collect(),
    }
}

'''
text = replace_once(text, marker, insert + marker, "storefront menu mapping")
path.write_text(text)

# Extend the source guard for the cache ownership boundary used by current main.
test_path = Path("crates/rustok-pages/storefront/tests/active_menu_transport_contract.rs")
test = test_path.read_text()
test = test.replace(
    '''        "MENU_LOCALE_NOT_FOUND_ERROR_CODE",
''',
    '''        "MENU_LOCALE_NOT_FOUND_ERROR_CODE",
        "cached.active_header_menu = active_header_menu.clone()",
        "cached.active_footer_menu = active_footer_menu.clone()",
        "cached_data.active_header_menu = None",
        "cached_data.active_footer_menu = None",
''',
    1,
)
test_path.write_text(test)
