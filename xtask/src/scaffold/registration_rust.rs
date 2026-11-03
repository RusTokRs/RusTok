use super::options::ScaffoldOptions;
use anyhow::{Context, Result};
use std::{fs, path::Path};
use syn::{
    punctuated::Punctuated, spanned::Spanned, visit::Visit, Expr, Item, ItemFn, Lit, Meta, Pat,
    Stmt, Token,
};

pub(super) const DISTRIBUTION_SOURCE: &str = "crates/modules/rustok-distribution/src/lib.rs";

pub(super) fn register(root: &Path, opts: &ScaffoldOptions, pascal: &str) -> Result<()> {
    let path = root.join(DISTRIBUTION_SOURCE);
    let mut source = fs::read_to_string(&path)?;
    let (file, expected) = parse(&source, opts, pascal)?;
    let registry = registry_function(&file)?;
    let tail = registry_tail(registry)?;
    let occupied = expected.usage(registry);
    anyhow::ensure!(
        occupied.registrations == 0 && occupied.features == 0,
        "Distribution registry already uses feature '{}' or module '{}::{}'",
        expected.feature,
        expected.crate_name,
        expected.module
    );

    // syn removes the BOM and shebang before assigning token spans.
    let prefix_bytes = usize::from(source.starts_with('\u{feff}')) * '\u{feff}'.len_utf8()
        + file.shebang.as_deref().map_or(0, str::len);
    let offset = prefix_bytes + tail.span().byte_range().start;
    let before = source
        .get(..offset)
        .context("Invalid distribution registry source position")?;
    let prefix = before.rsplit('\n').next().unwrap_or_default();
    let own_line = prefix.bytes().all(|b| b == b' ' || b == b'\t');
    let indent = if own_line { prefix } else { "    " };
    let nearby_eol = source[offset..]
        .find('\n')
        .map(|i| i + offset)
        .or_else(|| before.rfind('\n'));
    let eol = if nearby_eol.is_some_and(|i| i > 0 && source.as_bytes()[i - 1] == b'\r') {
        "\r\n"
    } else {
        "\n"
    };
    let leading = if own_line {
        String::new()
    } else {
        format!("{eol}{indent}")
    };
    let line = format!("{leading}#[cfg(feature = \"{}\")]{eol}{indent}let registry = registry.register({}::{});{eol}{indent}",
        expected.feature, expected.crate_name, expected.module);
    source.insert_str(offset, &line);
    fs::write(path, source)?;
    Ok(())
}

pub(super) fn validate(root: &Path, opts: &ScaffoldOptions, pascal: &str) -> Result<()> {
    let source = fs::read_to_string(root.join(DISTRIBUTION_SOURCE))?;
    let (file, expected) = parse(&source, opts, pascal)?;
    let registry = registry_function(&file)?;
    registry_tail(registry)?;
    let usage = expected.usage(registry);
    anyhow::ensure!(
        usage.registrations == 1
            && usage.features == 1
            && matches!(registry.block.stmts.as_slice(), [.., Stmt::Local(local), _]
            if matches!(&local.pat, Pat::Ident(binding) if binding.ident == "registry"
                && binding.by_ref.is_none() && binding.mutability.is_none() && binding.subpat.is_none())
            && matches!(local.attrs.as_slice(), [attr]
                if attr.path().is_ident("cfg") && attr.parse_args::<Meta>().is_ok_and(|meta| expected.is_feature(&meta)))
            && matches!(&local.init, Some(init) if init.diverge.is_none()
                && matches!(unparen(&init.expr), Expr::MethodCall(call)
                    if expected.is_registration(call) && is_path(&call.receiver, &["registry"])))),
        "Incomplete distribution registry entry for '{}'",
        opts.slug
    );
    Ok(())
}

fn parse(source: &str, opts: &ScaffoldOptions, pascal: &str) -> Result<(syn::File, Identity)> {
    Ok((
        syn::parse_file(source).context("Cannot parse distribution registry")?,
        Identity {
            crate_name: opts.crate_name().replace('-', "_"),
            module: format!("{pascal}Module"),
            feature: format!("mod-{}", opts.slug),
        },
    ))
}

fn registry_function(file: &syn::File) -> Result<&ItemFn> {
    let mut functions = file.items.iter().filter_map(|item| match item {
        Item::Fn(fun) if fun.sig.ident == "build_registry" => Some(fun),
        _ => None,
    });
    let function = functions
        .next()
        .context("Missing distribution build_registry function")?;
    anyhow::ensure!(
        functions.next().is_none(),
        "Ambiguous distribution build_registry function"
    );
    Ok(function)
}

fn registry_tail(function: &ItemFn) -> Result<&Expr> {
    match function.block.stmts.last() {
        Some(Stmt::Expr(expr, None))
            if matches!(unparen(expr), Expr::Call(call)
            if call.attrs.is_empty() && is_path(&call.func, &["generated_promotions", "register_promoted_modules"])
            && call.args.len() == 1 && is_path(&call.args[0], &["registry"])) =>
        {
            Ok(expr)
        }
        _ => anyhow::bail!("Missing distribution build_registry promotion tail"),
    }
}

fn unparen(mut expr: &Expr) -> &Expr {
    while let Expr::Paren(paren) = expr {
        expr = &paren.expr;
    }
    expr
}

fn is_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(unparen(expr), Expr::Path(path) if path.qself.is_none()
        && path.path.segments.len() == expected.len()
        && path.path.segments.iter().zip(expected).all(|(segment, name)|
            segment.ident == *name && segment.arguments.is_empty()))
}

struct Identity {
    crate_name: String,
    module: String,
    feature: String,
}

impl Identity {
    fn is_registration(&self, call: &syn::ExprMethodCall) -> bool {
        call.method == "register"
            && call.args.len() == 1
            && is_path(&call.args[0], &[&self.crate_name, &self.module])
    }

    fn is_feature(&self, meta: &Meta) -> bool {
        matches!(meta, Meta::NameValue(value) if value.path.is_ident("feature")
            && matches!(&value.value, Expr::Lit(lit) if matches!(&lit.lit, Lit::Str(name) if name.value() == self.feature)))
    }

    fn uses_feature(&self, meta: &Meta) -> bool {
        self.is_feature(meta)
            || matches!(meta, Meta::List(list)
            if list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(|inner| inner.iter().any(|meta| self.uses_feature(meta))))
    }

    fn usage(&self, registry: &ItemFn) -> Usage<'_> {
        let mut usage = Usage {
            identity: self,
            registrations: 0,
            features: 0,
        };
        usage.visit_item_fn(registry);
        usage
    }
}

struct Usage<'a> {
    identity: &'a Identity,
    registrations: usize,
    features: usize,
}

impl<'ast> Visit<'ast> for Usage<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.registrations += usize::from(self.identity.is_registration(call));
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
            self.features += usize::from(self.identity.uses_feature(&attr.meta));
        }
    }
}
