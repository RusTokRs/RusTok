from pathlib import Path


path = Path("/tmp/apply_pages_menu_layout_slots.py")
source = path.read_text()


def replace_call_by_label(source: str, label: str, replacement: str) -> str:
    label_index = source.index(f'    "{label}",')
    start = source.rfind("text = replace_once(\n", 0, label_index)
    if start < 0:
        raise RuntimeError(f"unable to locate start for {label}")
    end = source.find("text = replace_once(\n", label_index)
    if end < 0:
        raise RuntimeError(f"unable to locate end for {label}")
    return source[:start] + replacement + "\n" + source[end:]


source = replace_call_by_label(
    source,
    "generated component registrations",
    r'''registration_marker = """            route_segment = entry.route_segment,
            title = entry.page_title,
            fn_name = storefront_render_fn_name(&entry.slug),
        ));
"""
registration_replacement = registration_marker + """        for component in &entry.components {
            out.push_str(&format!(
                \"    register_component(StorefrontComponentRegistration {{ id: \\\"{id}\\\", module_slug: Some(\\\"{slug}\\\"), slot: {slot_expr}, order: {order}, render: {fn_name} }});\\n\",
                id = component.id,
                slug = entry.slug,
                slot_expr = storefront_slot_expr(component.slot),
                order = component.order,
                fn_name = storefront_component_render_fn_name(&entry.slug, &component.id),
            ));
        }
"""
text = replace_once(
    text,
    registration_marker,
    registration_replacement,
    "generated component registrations",
)''',
)

source = replace_call_by_label(
    source,
    "generated component render functions",
    r'''codegen_start = text.index("fn render_storefront_codegen")
codegen_end = text.index("fn storefront_slot_from_manifest", codegen_start)
codegen = text[codegen_start:codegen_end]
render_tail = """    }

    out
}

"""
render_replacement = """        for component in &entry.components {
            let fn_name = storefront_component_render_fn_name(&entry.slug, &component.id);
            let component_path = format!(\"{}::{}\", entry.crate_ident, component.component_name);
            out.push_str(&format!(\"fn {fn_name}() -> AnyView {{\\n\"));
            out.push_str(\"    view! {\\n\");
            out.push_str(&format!(\"        <{component_path} />\\n\"));
            out.push_str(\"    }\\n\");
            out.push_str(\"    .into_any()\\n\");
            out.push_str(\"}\\n\\n\");
        }
    }

    out
}

"""
codegen = replace_once(
    codegen,
    render_tail,
    render_replacement,
    "generated component render functions",
)
text = text[:codegen_start] + codegen + text[codegen_end:]''',
)

path.write_text(source)
