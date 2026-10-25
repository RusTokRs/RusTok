#!/usr/bin/env python3
"""One-shot H-1b transformer: DbBackend wildcard dispatch in crate
migrations -> exhaustive SupportedBackend dispatch.

Strict by design: any shape it does not recognize lands in the report
for hand conversion instead of being guessed at.
"""
import glob
import re
import sys

MIG = "crates/modules/rustok-modules/src/migrations"
HAND = {
    "m20260716_000014_artifact_secret_bindings.rs",      # done (H-1a)
    "m20260822_000044_module_build_execution_claims.rs",  # done (H-1a)
    "m20260720_000032_artifact_binding_operation_rls.rs",  # eq-only
    "m20260903_000047_artifact_data_copy_operations.rs",   # helper fns
    "m20260903_000048_artifact_data_object_copy_operations.rs",  # helper fns
    "m20260903_000049_artifact_data_snapshot_and_recovery_operations.rs",  # mixed
    "mod.rs",
}

ONLY = sys.argv[1:] or None


def split_regions(lines):
    """Split file lines at `async fn` boundaries (any indent)."""
    regions = [(0, [])]
    for i, line in enumerate(lines):
        if re.match(r"\s*async fn ", line) and i > 0:
            regions.append((i, []))
        regions[-1][1].append(line)
    return regions


def delete_arm_block(lines, start_idx):
    """Delete a `backend => { ... }` arm starting at start_idx.
    Returns the full lines list minus the block. The arm body is
    balanced-brace text ending with a `}` at the arm's indent."""
    indent = re.match(r"\s*", lines[start_idx]).group(0)
    depth = 0
    end = start_idx
    for j in range(start_idx, len(lines)):
        depth += lines[j].count("{") - lines[j].count("}")
        if j > start_idx and depth <= 0:
            end = j
            break
    return lines[:start_idx] + lines[end + 1:]


def transform_region(lines, report, fname):
    text = "".join(lines)
    needs_backend = (
        "match manager.get_database_backend()" in text
        or "manager.get_database_backend()," in text
        or "Statement::from_string(manager.get_database_backend()," in text
        or "Statement::from_sql_and_values(manager.get_database_backend()," in text
    )
    if not needs_backend:
        return lines
    # Find first line that opens the fn body (signature line ending with `{`
    # at column-ish) to insert the decl right after it.
    insert_at = None
    for i, line in enumerate(lines):
        if re.match(r"\s*async fn ", line):
            # body opens on this line or a following one ending in ` {`
            for j in range(i, min(i + 4, len(lines))):
                if lines[j].rstrip().endswith("{"):
                    insert_at = j + 1
                    break
            break
    if insert_at is None:
        report.append(f"{fname}: no fn body found for backend decl")
        return lines
    indent = re.match(r"\s*", lines[insert_at]).group(0) if insert_at < len(lines) else "        "
    decl = f"{indent}let backend = SupportedBackend::new(manager.get_database_backend())?;\n"
    lines = lines[:insert_at] + [decl] + lines[insert_at:]
    text = "".join(lines)
    text = text.replace(
        "match manager.get_database_backend()", "match backend"
    )
    text = text.replace(
        "Statement::from_string(manager.get_database_backend(),",
        "Statement::from_string(backend.db_backend(),",
    )
    text = text.replace(
        "Statement::from_sql_and_values(manager.get_database_backend(),",
        "Statement::from_sql_and_values(backend.db_backend(),",
    )
    text = re.sub(
        r"(\s+)manager\.get_database_backend\(\),", r"\1backend.db_backend(),", text
    )
    text = text.replace("DbBackend::Postgres =>", "SupportedBackend::Postgres =>")
    text = text.replace("DbBackend::Sqlite =>", "SupportedBackend::Sqlite =>")
    lines = text.splitlines(keepends=True)
    # Delete fallback arms (iterate from the end for stable indices).
    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if re.match(
            r'\s*_ => return Err\(DbErr::Custom\("Unsupported database backend"\.to_string\(\)\)\),\n?$',
            line,
        ):
            i += 1
            continue
        if re.match(r"\s*backend => \{\n?$", line):
            # delete until the closing `}` of this arm
            depth = 0
            j = i
            while j < len(lines):
                depth += lines[j].count("{") - lines[j].count("}")
                if j > i and depth <= 0:
                    break
                j += 1
            i = j + 1
            continue
        if re.match(r"\s*_ => &\[\n?$", line):
            if any("SupportedBackend::Sqlite =>" in l for l in lines):
                report.append(f"{fname}: wildcard slice arm with explicit Sqlite arm")
            line = line.replace("_ =>", "SupportedBackend::Sqlite =>", 1)
        out.append(line)
        i += 1
    return out


def transform_file(path, fname):
    report = []
    src = open(path).read()
    if "get_database_backend" not in src:
        return "skip", report
    lines = src.splitlines(keepends=True)
    regions = split_regions(lines)
    new_lines = []
    for _, region in regions:
        new_lines.extend(transform_region(region, report, fname))
    text = "".join(new_lines)
    if "use crate::backend::SupportedBackend;" not in text and "SupportedBackend" in text:
        if text.count("use sea_orm_migration::prelude::*;") != 1:
            report.append(f"{fname}: unexpected prelude import shape")
            return "report", report
        text = text.replace(
            "use sea_orm_migration::prelude::*;",
            "use sea_orm_migration::prelude::*;\n\nuse crate::backend::SupportedBackend;",
            1,
        )
    # Drop the now-unused DbBackend import when nothing references it.
    if "DbBackend::" not in text and "DbBackend)" not in text and "DbBackend," not in text.replace(
        "use sea_orm::", ""
    ):
        text = text.replace("DbBackend, ", "").replace(", DbBackend", "")
    leftovers = []
    for n, line in enumerate(text.splitlines(), 1):
        if "get_database_backend()" in line and "SupportedBackend::new(manager.get_database_backend())" not in line:
            leftovers.append(f"{fname}:{n}: leftover get_database_backend: {line.strip()}")
        if "DbBackend::" in line:
            leftovers.append(f"{fname}:{n}: leftover DbBackend:: {line.strip()}")
    report.extend(leftovers)
    if text != src:
        open(path, "w").write(text)
        return ("changed" if not leftovers else "changed+leftovers"), report
    return "unchanged", report


def main():
    files = sorted(glob.glob(f"{MIG}/m2026*.rs"))
    for path in files:
        fname = path.split("/")[-1]
        if fname in HAND:
            continue
        if ONLY and fname not in ONLY:
            continue
        status, report = transform_file(path, fname)
        print(f"{status}: {fname}")
        for item in report:
            print(f"    {item}")


if __name__ == "__main__":
    main()
