use std::collections::BTreeSet;

use proc_macro2::{Spacing, TokenTree};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Block, Expr, ExprCall, ExprMethodCall, FnArg, ImplItem, ItemImpl, Macro, Member,
    Meta, Pat, Path, Signature, Stmt, Token,
};

use super::Severity;

pub(super) struct AstFinding {
    pub line: usize,
    pub pattern_id: &'static str,
    pub rule_name: &'static str,
    pub severity: Severity,
    pub message: String,
    pub suggested_fix: String,
}

pub(super) fn scan(source: &str) -> Vec<AstFinding> {
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };

    let mut visitor = AstVisitor::default();
    visitor.visit_file(&file);
    visitor.findings
}

#[derive(Default)]
struct AstVisitor {
    findings: Vec<AstFinding>,
}

impl AstVisitor {
    fn add(
        &mut self,
        line: usize,
        pattern_id: &'static str,
        rule_name: &'static str,
        severity: Severity,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) {
        self.findings.push(AstFinding {
            line,
            pattern_id,
            rule_name,
            severity,
            message: message.into(),
            suggested_fix: suggested_fix.into(),
        });
    }

    fn check_ffi_boundary(&mut self, signature: &Signature, block: &Block) {
        if !is_non_unwind_abi(signature) {
            return;
        }

        let mut panic_risk = PanicRiskVisitor::default();
        panic_risk.visit_block(block);
        let Some(operation) = panic_risk.operation else {
            return;
        };

        self.add(
            signature.span().start().line,
            "P-RUST-FFI-UNWIND-BOUNDARY",
            "Panic-Capable Operation at Non-Unwinding FFI Boundary",
            Severity::High,
            format!(
                "Non-unwinding foreign ABI function contains '{operation}'. A Rust panic reaching this boundary aborts rather than propagating to the caller."
            ),
            "Return an explicit status/error across the ABI boundary. Catch Rust panics only when the boundary contract defines a safe recovery policy; do not allow foreign exceptions to enter through a non-unwinding ABI.",
        );
    }

    fn check_unsafe_send_sync(&mut self, item: &ItemImpl) {
        if item.unsafety.is_none() {
            return;
        }

        let Some((trait_path, _)) = item.trait_.as_ref() else {
            return;
        };
        let Some(trait_segment) = trait_path.segments.last() else {
            return;
        };
        let trait_name = trait_segment.ident.to_string();
        if trait_name != "Send" && trait_name != "Sync" {
            return;
        }

        self.add(
            item.span().start().line,
            "P-RUST-UNSAFE-SEND-SYNC-CONTRACT",
            "Manual Unsafe Send/Sync Contract",
            Severity::High,
            format!(
                "Manual 'unsafe impl {trait_name}' overrides compiler-derived thread-safety guarantees. Verify ownership, aliasing, interior mutability, synchronization, and destruction for every reachable value."
            ),
            "Prefer compiler-derived Send/Sync. If a manual implementation is necessary, document the complete safety invariant and add focused compile-time and concurrency/UB regression coverage. A SAFETY comment alone is not proof.",
        );
    }

    fn check_lock_order(&mut self, item: &ItemImpl) {
        let mut methods = Vec::<(String, Vec<LockEdge>)>::new();

        for impl_item in &item.items {
            let ImplItem::Fn(method) = impl_item else {
                continue;
            };
            if is_test_item(&method.attrs)
                || !method
                    .sig
                    .inputs
                    .iter()
                    .any(|argument| matches!(argument, FnArg::Receiver(_)))
            {
                continue;
            }

            let edges = lock_order_edges(&method.block);
            if !edges.is_empty() {
                methods.push((method.sig.ident.to_string(), edges));
            }
        }

        let mut reported = BTreeSet::new();
        for left_index in 0..methods.len() {
            for right_index in (left_index + 1)..methods.len() {
                let (left_method, left_edges) = &methods[left_index];
                let (right_method, right_edges) = &methods[right_index];

                for left in left_edges {
                    let Some(right) = right_edges
                        .iter()
                        .find(|right| right.from == left.to && right.to == left.from)
                    else {
                        continue;
                    };

                    let (first, second) = if left.from <= left.to {
                        (left.from.clone(), left.to.clone())
                    } else {
                        (left.to.clone(), left.from.clone())
                    };
                    if !reported.insert((right.line, first, second)) {
                        continue;
                    }

                    self.add(
                        right.line,
                        "P-RUST-LOCK-ORDER-CYCLE",
                        "Opposite Lock Acquisition Order",
                        Severity::High,
                        format!(
                            "Methods '{left_method}' and '{right_method}' use lock-like methods on the same self-owned fields in opposite orders: '{}' before '{}' versus '{}' before '{}'. Confirm the receiver types and that the guards overlap on a concurrent execution path.",
                            left.from, left.to, right.from, right.to
                        ),
                        "Use one canonical order for acquiring these locks. If atomic overlap is unnecessary, release the first guard before acquiring the second.",
                    );
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for AstVisitor {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let name = item.ident.to_string();
        if is_test_item(&item.attrs) || name == "test" || name == "tests" {
            return;
        }
        visit::visit_item_mod(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if is_test_item(&item.attrs) {
            return;
        }
        self.check_ffi_boundary(&item.sig, &item.block);
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if is_test_item(&item.attrs) {
            return;
        }
        self.check_ffi_boundary(&item.sig, &item.block);
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        if is_test_item(&item.attrs) {
            return;
        }
        self.check_unsafe_send_sync(item);
        self.check_lock_order(item);
        visit::visit_item_impl(self, item);
    }

    fn visit_macro(&mut self, item: &'ast Macro) {
        if last_segment(&item.path).as_deref() == Some("select") {
            if let Some(method) = select_cancel_unsafe_method(&item.tokens) {
                self.add(
                    item.span().start().line,
                    "P-RUST-SELECT-CANCEL-UNSAFE",
                    "Cancellation-Unsafe Future in select! Branch",
                    Severity::High,
                    format!(
                        "A select! branch future calls '{method}', matching a Tokio API documented as cancellation-unsafe. Receiver types are not resolved, so verify this candidate. A losing branch is dropped when another branch completes."
                    ),
                    "Use a cancellation-safe primitive, preserve partial progress explicitly, or move the operation into a task whose owner controls completion and cancellation.",
                );
            }
        }
        visit::visit_macro(self, item);
    }
}

#[derive(Default)]
struct PanicRiskVisitor {
    operation: Option<&'static str>,
}

impl<'ast> Visit<'ast> for PanicRiskVisitor {
    fn visit_item(&mut self, _item: &'ast syn::Item) {}

    fn visit_item_fn(&mut self, _item: &'ast syn::ItemFn) {}

    fn visit_impl_item_fn(&mut self, _item: &'ast syn::ImplItemFn) {}

    fn visit_item_impl(&mut self, _item: &'ast ItemImpl) {}

    fn visit_expr_closure(&mut self, _item: &'ast syn::ExprClosure) {}

    fn visit_expr_async(&mut self, _item: &'ast syn::ExprAsync) {}

    fn visit_macro(&mut self, item: &'ast Macro) {
        if self.operation.is_none() {
            if let Some(name) = last_segment(&item.path) {
                self.operation = panic_macro(&name);
            }
        }
        visit::visit_macro(self, item);
    }

    fn visit_expr_method_call(&mut self, item: &'ast ExprMethodCall) {
        if self.operation.is_none() {
            self.operation = match item.method.to_string().as_str() {
                "unwrap" => Some(".unwrap()"),
                "expect" => Some(".expect()"),
                _ => None,
            };
        }
        visit::visit_expr_method_call(self, item);
    }

    fn visit_expr_call(&mut self, item: &'ast ExprCall) {
        if self.operation.is_none() {
            if let Expr::Path(function) = item.func.as_ref() {
                if last_segment(&function.path).as_deref() == Some("panic_any") {
                    self.operation = Some("panic_any()");
                }
            }
        }
        visit::visit_expr_call(self, item);
    }
}

fn panic_macro(name: &str) -> Option<&'static str> {
    match name {
        "panic" => Some("panic!"),
        "assert" => Some("assert!"),
        "assert_eq" => Some("assert_eq!"),
        "assert_ne" => Some("assert_ne!"),
        "debug_assert" => Some("debug_assert!"),
        "debug_assert_eq" => Some("debug_assert_eq!"),
        "debug_assert_ne" => Some("debug_assert_ne!"),
        "todo" => Some("todo!"),
        "unimplemented" => Some("unimplemented!"),
        "unreachable" => Some("unreachable!"),
        _ => None,
    }
}

fn is_non_unwind_abi(signature: &Signature) -> bool {
    let Some(abi) = signature.abi.as_ref() else {
        return false;
    };

    match abi.name.as_ref().map(|name| name.value()) {
        Some(name) => name != "Rust" && !name.ends_with("-unwind"),
        None => true,
    }
}

fn is_test_item(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident.to_string() == "test")
        {
            return true;
        }

        if !attribute.path().is_ident("cfg") {
            return false;
        }

        let Meta::List(list) = &attribute.meta else {
            return false;
        };
        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        parser
            .parse2(list.tokens.clone())
            .map(|nested| nested.iter().any(meta_contains_test))
            .unwrap_or(false)
    })
}

fn meta_contains_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) => {
            if list.path.is_ident("not") {
                return false;
            }
            let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
            parser
                .parse2(list.tokens.clone())
                .map(|nested| nested.iter().any(meta_contains_test))
                .unwrap_or(false)
        }
        _ => false,
    }
}

fn last_segment(path: &Path) -> Option<String> {
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn select_cancel_unsafe_method(tokens: &proc_macro2::TokenStream) -> Option<&'static str> {
    let tokens: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut branch_start = 0;
    let mut cursor = 0;

    while cursor + 1 < tokens.len() {
        let Some(arrow) =
            (cursor..tokens.len() - 1).find(|index| is_fat_arrow(&tokens, *index))
        else {
            break;
        };

        if let Some(method) = unsafe_method_in_branch(&tokens[branch_start..arrow]) {
            return Some(method);
        }

        cursor = arrow + 2;
        let Some(separator) =
            (cursor..tokens.len()).find(|index| is_punct(&tokens[*index], ','))
        else {
            break;
        };
        branch_start = separator + 1;
        cursor = branch_start;
    }

    None
}

fn unsafe_method_in_branch(branch: &[TokenTree]) -> Option<&'static str> {
    let assignment = branch
        .iter()
        .position(|token| is_punct(token, '='))?;
    let mut expression_end = branch.len();

    for index in assignment + 1..branch.len() {
        if is_punct(&branch[index], ',')
            && matches!(branch.get(index + 1), Some(TokenTree::Ident(ident)) if ident.to_string() == "if")
        {
            expression_end = index;
            break;
        }
    }

    method_call_in_tokens(&branch[assignment + 1..expression_end])
}

fn method_call_in_tokens(tokens: &[TokenTree]) -> Option<&'static str> {
    for (index, token) in tokens.iter().enumerate() {
        if is_punct(token, '.') {
            if let Some(TokenTree::Ident(method)) = tokens.get(index + 1) {
                if let Some(unsafe_method) = cancel_unsafe_method(&method.to_string()) {
                    return Some(unsafe_method);
                }
            }
        }

        if let TokenTree::Group(group) = token {
            let nested: Vec<TokenTree> = group.stream().into_iter().collect();
            if let Some(method) = method_call_in_tokens(&nested) {
                return Some(method);
            }
        }
    }
    None
}

fn cancel_unsafe_method(method: &str) -> Option<&'static str> {
    match method {
        "read_exact" => Some("read_exact"),
        "read_to_end" => Some("read_to_end"),
        "read_to_string" => Some("read_to_string"),
        "write_all" => Some("write_all"),
        "lock" => Some("lock"),
        "acquire" => Some("acquire"),
        "notified" => Some("notified"),
        _ => None,
    }
}

fn is_fat_arrow(tokens: &[TokenTree], index: usize) -> bool {
    matches!(
        (tokens.get(index), tokens.get(index + 1)),
        (
            Some(TokenTree::Punct(equals)),
            Some(TokenTree::Punct(greater))
        ) if equals.as_char() == '='
            && equals.spacing() == Spacing::Joint
            && greater.as_char() == '>'
    )
}

fn is_punct(token: &TokenTree, character: char) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == character)
}

#[derive(Clone, Debug)]
struct LockEdge {
    from: String,
    to: String,
    line: usize,
}

fn lock_order_edges(block: &Block) -> Vec<LockEdge> {
    let mut held = Vec::<(String, String)>::new();
    let mut edges = Vec::new();

    for statement in &block.stmts {
        match statement {
            Stmt::Local(local) => {
                let Some(initializer) = local.init.as_ref() else {
                    continue;
                };
                let Some(target) = lock_target(&initializer.expr) else {
                    continue;
                };
                let Some(guard) = binding_name(&local.pat) else {
                    continue;
                };

                held.retain(|(name, _)| name != &guard);
                for (_, held_target) in &held {
                    if held_target != &target {
                        edges.push(LockEdge {
                            from: held_target.clone(),
                            to: target.clone(),
                            line: initializer.expr.span().start().line,
                        });
                    }
                }
                held.push((guard, target));
            }
            Stmt::Expr(expression, _) => {
                if let Some(guard) = dropped_guard(expression) {
                    held.retain(|(name, _)| name != &guard);
                }
            }
            _ => {}
        }
    }

    edges
}

fn binding_name(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Ident(binding) => Some(binding.ident.to_string()),
        Pat::Type(typed) => binding_name(&typed.pat),
        Pat::Paren(paren) => binding_name(&paren.pat),
        _ => None,
    }
}

fn lock_target(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Await(awaited) => lock_target(&awaited.base),
        Expr::Try(tried) => lock_target(&tried.expr),
        Expr::Paren(paren) => lock_target(&paren.expr),
        Expr::Group(group) => lock_target(&group.expr),
        Expr::Block(block) => block.block.stmts.iter().rev().find_map(|statement| match statement {
            Stmt::Expr(expression, None) => lock_target(expression),
            _ => None,
        }),
        Expr::MethodCall(call) => {
            let method = call.method.to_string();
            if is_lock_method(&method) {
                self_field_path(&call.receiver)
            } else if matches!(
                method.as_str(),
                "unwrap" | "expect" | "unwrap_or_else" | "map_err" | "ok_or_else"
            ) {
                lock_target(&call.receiver)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_lock_method(method: &str) -> bool {
    matches!(
        method,
        "lock"
            | "try_lock"
            | "lock_shared"
            | "lock_exclusive"
            | "try_lock_shared"
            | "try_lock_exclusive"
            | "read"
            | "try_read"
            | "write"
            | "try_write"
    )
}

fn self_field_path(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Path(path) if path.path.is_ident("self") => Some("self".to_owned()),
        Expr::Field(field) => {
            let base = self_field_path(&field.base)?;
            let member = match &field.member {
                Member::Named(ident) => ident.to_string(),
                Member::Unnamed(index) => index.index.to_string(),
            };
            Some(format!("{base}.{member}"))
        }
        Expr::Paren(paren) => self_field_path(&paren.expr),
        Expr::Group(group) => self_field_path(&group.expr),
        Expr::Reference(reference) => self_field_path(&reference.expr),
        Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "clone" | "as_ref" | "as_mut" | "borrow" | "deref"
            ) =>
        {
            self_field_path(&call.receiver)
        }
        _ => None,
    }
}

fn dropped_guard(expression: &Expr) -> Option<String> {
    let Expr::Call(call) = expression else {
        return None;
    };
    let Expr::Path(function) = call.func.as_ref() else {
        return None;
    };
    if last_segment(&function.path).as_deref() != Some("drop") {
        return None;
    }

    let argument = call.args.first()?;
    let Expr::Path(path) = argument else {
        return None;
    };
    if path.path.segments.len() != 1 {
        return None;
    }
    path.path
        .segments
        .first()
        .map(|segment| segment.ident.to_string())
}
