//! Derive macros for rustok-revisions.
//!
//! This crate provides derive macros to automatically implement
//! the `Revisionable` trait for your types.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for the `Revisionable` trait.
///
/// This macro automatically implements the `Revisionable` trait for your type,
/// providing the `content_type()` and `to_revision_json()` methods.
///
/// # Example
///
/// ```rust,ignore
/// use rustok_revisions::Revisionable;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Serialize, Deserialize, Revisionable)]
/// #[revision(content_type = "blog_post")]
/// struct Post {
///     title: String,
///     content: String,
///     views: i32,
/// }
/// ```
///
/// This will generate:
///
/// ```rust,ignore
/// impl Revisionable for Post {
///     fn content_type() -> &'static str {
///         "blog_post"
///     }
///
///     fn to_revision_json(&self) -> serde_json::Value {
///         serde_json::to_value(self).unwrap()
///     }
/// }
/// ```
#[proc_macro_derive(Revisionable, attributes(revision))]
pub fn derive_revisionable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Parse attributes to get content_type
    let mut content_type = None;
    for attr in &input.attrs {
        if attr.path().is_ident("revision") {
            if let Ok(meta) = attr.parse_args::<syn::Meta>() {
                if let syn::Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("content_type") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = &nv.value
                        {
                            content_type = Some(s.value());
                        }
                    }
                }
            }
        }
    }

    // Default content_type to lowercase struct name
    let content_type_str = content_type.unwrap_or_else(|| name.to_string().to_lowercase());

    let expanded = quote! {
        impl rustok_revisions::Revisionable for #name {
            fn content_type() -> &'static str {
                #content_type_str
            }

            fn to_revision_json(&self) -> serde_json::Value {
                serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
            }
        }
    };

    TokenStream::from(expanded)
}
