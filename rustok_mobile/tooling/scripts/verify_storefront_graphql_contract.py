#!/usr/bin/env python3
"""Verify Flutter storefront GraphQL documents against server-owned surfaces.

The check is intentionally source-backed: it does not require a Flutter SDK or a
running RusTok server, but it validates that mobile operation documents keep
matching the existing storefront/search APIs and commerce runtime parity flow.
"""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Literal, TypedDict


class ContractEvidence(TypedDict):
    const: str
    operation: str
    kind: str
    root_field: str
    server_evidence: list[str]


class LiveExecutionEvidence(TypedDict):
    operation: str
    root_field: str
    status: Literal["passed", "skipped"]
    source: str
    message: str


@dataclass(frozen=True)
class SourceMarker:
    path: str
    marker: str


@dataclass(frozen=True)
class GraphQlContract:
    const_name: str
    operation_name: str
    operation_kind: str
    root_field: str
    server_markers: tuple[SourceMarker, ...]
    runtime_builder: str | None = None
    runtime_error_marker: str | None = None


SEARCH_STOREFRONT_API = "crates/rustok-search/storefront/src/api.rs"
COMMERCE_QUERY = "crates/rustok-commerce/src/graphql/query.rs"
COMMERCE_MUTATION = "crates/rustok-commerce/src/graphql/mutations/cart.rs"
COMMERCE_TYPES = "crates/rustok-commerce/src/graphql/types.rs"
COMMERCE_RUNTIME_TEST = "crates/rustok-commerce/tests/graphql_runtime_parity_test/main.rs"
COMMERCE_RUNTIME_CART_TEST = "crates/rustok-commerce/tests/graphql_runtime_parity_test/cart.rs"


CONTRACTS: tuple[GraphQlContract, ...] = (
    GraphQlContract(
        const_name="storefrontMobileCatalogQuery",
        operation_name="StorefrontMobileCatalog",
        operation_kind="query",
        root_field="storefrontSearch",
        server_markers=(
            SourceMarker(
                SEARCH_STOREFRONT_API,
                "query StorefrontSearch($input: SearchPreviewInput!)",
            ),
            SourceMarker(SEARCH_STOREFRONT_API, "storefrontSearch(input: $input)"),
            SourceMarker(SEARCH_STOREFRONT_API, "struct SearchPreviewInput"),
        ),
    ),
    GraphQlContract(
        const_name="storefrontMobileCartQuery",
        operation_name="StorefrontMobileCart",
        operation_kind="query",
        root_field="storefrontCart",
        server_markers=(
            SourceMarker(COMMERCE_QUERY, "async fn storefront_cart"),
            SourceMarker(COMMERCE_QUERY, "id: Uuid"),
        ),
        runtime_builder="storefront_cart_query",
        runtime_error_marker="unexpected cart query GraphQL errors",
    ),
    GraphQlContract(
        const_name="storefrontMobileCreateCartMutation",
        operation_name="StorefrontMobileCreateCart",
        operation_kind="mutation",
        root_field="createStorefrontCart",
        server_markers=(
            SourceMarker(COMMERCE_MUTATION, "async fn create_storefront_cart"),
            SourceMarker(COMMERCE_TYPES, "pub struct CreateStorefrontCartInput"),
        ),
        runtime_builder="storefront_cart_flow_mutation",
        runtime_error_marker="unexpected create cart GraphQL errors",
    ),
    GraphQlContract(
        const_name="storefrontMobileAddCartLineMutation",
        operation_name="StorefrontMobileAddCartLine",
        operation_kind="mutation",
        root_field="addStorefrontCartLineItem",
        server_markers=(
            SourceMarker(COMMERCE_MUTATION, "async fn add_storefront_cart_line_item"),
            SourceMarker(COMMERCE_TYPES, "pub struct AddStorefrontCartLineItemInput"),
        ),
        runtime_builder="storefront_cart_add_line_item_mutation",
        runtime_error_marker="unexpected add line item GraphQL errors",
    ),
    GraphQlContract(
        const_name="storefrontMobileUpdateCartLineMutation",
        operation_name="StorefrontMobileUpdateCartLine",
        operation_kind="mutation",
        root_field="updateStorefrontCartLineItem",
        server_markers=(
            SourceMarker(
                COMMERCE_MUTATION,
                "async fn update_storefront_cart_line_item",
            ),
            SourceMarker(
                COMMERCE_TYPES,
                "pub struct UpdateStorefrontCartLineItemInput",
            ),
        ),
        runtime_builder="storefront_cart_update_line_item_mutation",
        runtime_error_marker="unexpected update line item GraphQL errors",
    ),
    GraphQlContract(
        const_name="storefrontMobileRemoveCartLineMutation",
        operation_name="StorefrontMobileRemoveCartLine",
        operation_kind="mutation",
        root_field="removeStorefrontCartLineItem",
        server_markers=(
            SourceMarker(
                COMMERCE_MUTATION,
                "async fn remove_storefront_cart_line_item",
            ),
            SourceMarker(COMMERCE_MUTATION, "line_id: Uuid"),
        ),
        runtime_builder="storefront_cart_remove_line_item_mutation",
        runtime_error_marker="unexpected remove line item GraphQL errors",
    ),
)

MOBILE_REPOSITORY_PATH = Path(
    "rustok_mobile/apps/rustok_frontend_mobile/lib/data/"
    "storefront_catalog_repository.dart"
)
MOBILE_CONTEXT_PATH = Path(
    "rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart"
)
COMMERCE_RUNTIME_TEST_PATH = Path(COMMERCE_RUNTIME_TEST)
COMMERCE_RUNTIME_CART_TEST_PATH = Path(COMMERCE_RUNTIME_CART_TEST)
FORBIDDEN_TRANSPORT_MARKERS = ("/api/flutter", "/api/mobile")
FORBIDDEN_DOCUMENT_MARKERS = (*FORBIDDEN_TRANSPORT_MARKERS, "tenantId:", "$tenantId")


class ContractError(RuntimeError):
    pass


def contract_key(contract: ContractEvidence) -> tuple[str, str]:
    return (contract["operation"], contract["root_field"])


def load_live_execution_results(
    evidence: list[ContractEvidence],
    live_results_path: Path | None,
) -> list[LiveExecutionEvidence]:
    """Attach optional live schema/test-server evidence to source checks."""
    if live_results_path is None:
        return [
            {
                "operation": contract["operation"],
                "root_field": contract["root_field"],
                "status": "skipped",
                "source": "preflight",
                "message": "live schema/test-server harness was not provided",
            }
            for contract in evidence
        ]

    raw = json.loads(live_results_path.read_text(encoding="utf-8"))
    results = raw.get("storefront_live_execution")
    if not isinstance(results, list):
        raise ContractError(
            "live results must contain a `storefront_live_execution` list"
        )

    by_key: dict[tuple[str, str], dict[str, object]] = {}
    for result in results:
        if not isinstance(result, dict):
            raise ContractError("each live execution result must be an object")
        operation = result.get("operation")
        root_field = result.get("root_field")
        status = result.get("status")
        if not isinstance(operation, str) or not isinstance(root_field, str):
            raise ContractError(
                "live execution result must include string operation/root_field"
            )
        if status != "passed":
            raise ContractError(
                f"live execution for `{operation}`/`{root_field}` did not pass"
            )
        by_key[(operation, root_field)] = result

    live_evidence: list[LiveExecutionEvidence] = []
    for contract in evidence:
        key = contract_key(contract)
        result = by_key.get(key)
        if result is None:
            raise ContractError(
                "missing live execution result for "
                f"`{contract['operation']}`/`{contract['root_field']}`"
            )
        source = result.get("source")
        message = result.get("message")
        live_evidence.append(
            {
                "operation": contract["operation"],
                "root_field": contract["root_field"],
                "status": "passed",
                "source": source if isinstance(source, str) else "live-harness",
                "message": message if isinstance(message, str) else "live execution passed",
            }
        )
    return live_evidence


def read(repo_root: Path, path: Path | str) -> str:
    return (repo_root / path).read_text(encoding="utf-8")


def extract_dart_raw_string_const(source: str, const_name: str) -> str:
    pattern = re.compile(
        rf"const\s+{re.escape(const_name)}\s*=\s*r'''(?P<body>.*?)''';",
        re.DOTALL,
    )
    match = pattern.search(source)
    if match is None:
        raise ContractError(f"missing Dart GraphQL const `{const_name}`")
    return match.group("body")


def assert_contains(source: str, marker: str, context: str) -> None:
    if marker not in source:
        raise ContractError(f"missing marker `{marker}` in {context}")


def assert_absent(source: str, marker: str, context: str) -> None:
    if marker in source:
        raise ContractError(f"forbidden marker `{marker}` in {context}")


def assert_runtime_builder_is_executed(
    runtime_builder_source: str,
    runtime_execution_source: str,
    builder: str,
) -> None:
    assert_contains(runtime_builder_source, f"fn {builder}", COMMERCE_RUNTIME_TEST)
    pattern = re.compile(rf"\.execute\(Request::new\(\s*{builder}")
    if pattern.search(runtime_execution_source) is None:
        raise ContractError(
            f"runtime parity test defines `{builder}` but does not execute it "
            "through schema.execute"
        )


def verify_contract(
    repo_root: Path,
    contract: GraphQlContract,
    mobile_source: str,
) -> ContractEvidence:
    document = extract_dart_raw_string_const(mobile_source, contract.const_name)
    assert_contains(
        document,
        f"{contract.operation_kind} {contract.operation_name}",
        contract.const_name,
    )
    assert_contains(document, f"{contract.root_field}(", contract.const_name)

    for forbidden in FORBIDDEN_DOCUMENT_MARKERS:
        assert_absent(document, forbidden, contract.const_name)

    checked_paths: set[str] = set()
    for source_marker in contract.server_markers:
        server_source = read(repo_root, source_marker.path)
        assert_contains(server_source, source_marker.marker, source_marker.path)
        checked_paths.add(source_marker.path)

    if contract.runtime_builder is not None:
        runtime_builder_source = read(repo_root, COMMERCE_RUNTIME_TEST_PATH)
        runtime_execution_source = read(repo_root, COMMERCE_RUNTIME_CART_TEST_PATH)
        assert_runtime_builder_is_executed(
            runtime_builder_source,
            runtime_execution_source,
            contract.runtime_builder,
        )
        checked_paths.add(COMMERCE_RUNTIME_TEST)
        checked_paths.add(COMMERCE_RUNTIME_CART_TEST)
    if contract.runtime_error_marker is not None:
        runtime_execution_source = read(repo_root, COMMERCE_RUNTIME_CART_TEST_PATH)
        assert_contains(
            runtime_execution_source,
            contract.runtime_error_marker,
            COMMERCE_RUNTIME_CART_TEST,
        )
        checked_paths.add(COMMERCE_RUNTIME_CART_TEST)

    return {
        "const": contract.const_name,
        "operation": contract.operation_name,
        "kind": contract.operation_kind,
        "root_field": contract.root_field,
        "server_evidence": sorted(checked_paths),
    }


def verify(repo_root: Path) -> list[ContractEvidence]:
    mobile_source = read(repo_root, MOBILE_REPOSITORY_PATH)
    mobile_context = read(repo_root, MOBILE_CONTEXT_PATH)

    for forbidden in FORBIDDEN_TRANSPORT_MARKERS:
        assert_absent(mobile_source, forbidden, str(MOBILE_REPOSITORY_PATH))
        assert_absent(mobile_context, forbidden, str(MOBILE_CONTEXT_PATH))

    assert_contains(
        mobile_source,
        "GraphQlStorefrontCatalogRepository",
        str(MOBILE_REPOSITORY_PATH),
    )
    assert_contains(
        mobile_context,
        "GraphQlClientFactory().create",
        str(MOBILE_CONTEXT_PATH),
    )

    return [
        verify_contract(repo_root, contract, mobile_source)
        for contract in CONTRACTS
    ]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path.cwd(),
        help="Repository root. Defaults to the current working directory.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print machine-readable evidence instead of a short OK line.",
    )
    parser.add_argument(
        "--live-results",
        type=Path,
        help=(
            "Optional JSON evidence produced by a live schema/test-server "
            "harness. The file must report a passed result for every "
            "source-verified storefront mobile operation."
        ),
    )
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    repo_root = args.repo_root.resolve()
    try:
        evidence = verify(repo_root)
        live_evidence = load_live_execution_results(evidence, args.live_results)
    except (ContractError, OSError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}")
        return 1

    if args.json:
        print(
            json.dumps(
                {
                    "storefront_graphql_contracts": evidence,
                    "storefront_live_execution": live_evidence,
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        live_passed = sum(1 for item in live_evidence if item["status"] == "passed")
        suffix = (
            f"; live execution passed for {live_passed} contracts"
            if live_passed
            else "; live execution skipped"
        )
        print(f"OK: verified {len(evidence)} storefront mobile GraphQL contracts{suffix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
