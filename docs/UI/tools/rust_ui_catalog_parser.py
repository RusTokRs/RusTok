#!/usr/bin/env python3
"""Parse Rust/UI navigation dump into a normalized component catalog markdown.

Usage:
  python docs/UI/tools/rust_ui_catalog_parser.py --input rust_ui_dump.txt --output docs/UI/rust-ui-component-catalog.md
"""
from __future__ import annotations

import argparse
from pathlib import Path

SKIP = {
    "Logo Rust/UI", "Docs", "Components", "Hooks", "Icons", "Themes", "Blocks", "Charts",
    "Get Started", "Introduction", "Installation", "CLI", "Figma", "Changelog", "Home",
    "View Markdown", "Open in ChatGPT", "Open in Claude", "Search documentation...", "Pages", "Navigate", "Go to Page",
}


def parse_components(text: str) -> list[str]:
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    out: list[str] = []
    seen = set()
    in_components = False
    for ln in lines:
        if ln == "Components":
            in_components = True
            continue
        if not in_components:
            continue
        if ln in SKIP:
            continue
        if ln.startswith("Get notified") or ln.startswith("Enter your email") or ln.startswith("On This Page"):
            continue
        if len(ln) > 64:
            continue
        if ln not in seen:
            seen.add(ln)
            out.append(ln)
    return out


def render_markdown(components: list[str]) -> str:
    rows = "\n".join(f"| {name} | TODO | TODO |" for name in components)
    return (
        "# Rust/UI Component Catalog (Local Snapshot)\n\n"
        "Автогенерируемый список компонентов Rust/UI для внутреннего планирования паритета.\n\n"
        "| Component | Decision (adopt/pilot/reject) | Target shared crate |\n"
        "| --- | --- | --- |\n"
        f"{rows}\n"
    )


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", required=True)
    ap.add_argument("--output", required=True)
    args = ap.parse_args()

    src = Path(args.input).read_text(encoding="utf-8")
    components = parse_components(src)
    md = render_markdown(components)
    Path(args.output).write_text(md, encoding="utf-8")
    print(f"Parsed {len(components)} components -> {args.output}")


if __name__ == "__main__":
    main()
