use rustok_ui_core::UiRouteContext;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminRoute {
    Catalog,
    Categories,
    Attributes,
    NewProduct,
    EditProduct(String),
}

impl ProductAdminRoute {
    pub(crate) fn resolve(context: &UiRouteContext) -> Self {
        // `products` is the declared landing child page; naming it keeps the
        // manifest and the dispatch in the same vocabulary instead of relying on
        // the fallthrough.
        if context.subpath_matches("products") {
            return Self::Catalog;
        }
        if context.subpath_matches("categories") {
            return Self::Categories;
        }
        if context.subpath_matches("attributes") {
            return Self::Attributes;
        }
        if context.subpath_matches("new") {
            return Self::NewProduct;
        }

        context
            .subpath()
            .and_then(product_id_from_subpath)
            .or_else(|| context.query_value("id"))
            .or_else(|| context.query_value("product_id"))
            .map_or(Self::Catalog, |id| Self::EditProduct(id.to_owned()))
    }
}

fn product_id_from_subpath(subpath: &str) -> Option<&str> {
    let subpath = subpath.trim_matches('/');
    if subpath.starts_with("edit/") {
        let id = subpath.trim_start_matches("edit/").trim_matches('/');
        if !id.is_empty() {
            return Some(id);
        }
    }
    uuid::Uuid::parse_str(subpath).ok().map(|_| subpath)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_mounted_admin_pages() {
        let product_id = "187dbd94-ba86-4dfe-b5ae-20d41d479e96";
        for (subpath, expected) in [
            (None, ProductAdminRoute::Catalog),
            (Some("categories/featured"), ProductAdminRoute::Categories),
            (Some("attributes"), ProductAdminRoute::Attributes),
            (Some("new"), ProductAdminRoute::NewProduct),
            (
                Some("/edit/product-1/"),
                ProductAdminRoute::EditProduct("product-1".into()),
            ),
            (
                Some(product_id),
                ProductAdminRoute::EditProduct(product_id.into()),
            ),
            (Some("unknown"), ProductAdminRoute::Catalog),
        ] {
            let context = UiRouteContext {
                subpath: subpath.map(str::to_owned),
                ..Default::default()
            };
            assert_eq!(ProductAdminRoute::resolve(&context), expected);
        }
    }

    #[test]
    fn prioritizes_mounted_path_over_query_ids() {
        let mut context = UiRouteContext {
            query: [
                ("id".into(), "primary".into()),
                ("product_id".into(), "fallback".into()),
            ]
            .into(),
            ..Default::default()
        };
        assert_eq!(
            ProductAdminRoute::resolve(&context),
            ProductAdminRoute::EditProduct("primary".into())
        );
        context.subpath = Some("edit/from-path".into());
        assert_eq!(
            ProductAdminRoute::resolve(&context),
            ProductAdminRoute::EditProduct("from-path".into())
        );
        context.subpath = Some("new".into());
        assert_eq!(
            ProductAdminRoute::resolve(&context),
            ProductAdminRoute::NewProduct
        );
        context.subpath = None;
        context.query.remove("id");
        assert_eq!(
            ProductAdminRoute::resolve(&context),
            ProductAdminRoute::EditProduct("fallback".into())
        );
    }
}
