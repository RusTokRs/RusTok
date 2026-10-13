#!/usr/bin/env python3
"""Verify the CLDR plural table in `cldr_plurals.dart` without a Dart SDK.

The table is a `switch (lang)` whose case bodies are `if (condition) return
'category';` statements. This script

1. reads the switch out of `lib/src/resolver/cldr_plurals.dart`,
2. checks it structurally: every case body ends in `return`/`break`/`continue`/
   `throw` (Dart does not allow an implicit fall-through, and a body that can
   complete normally makes the function miss its `String` return),
3. translates the conditions to Python and evaluates them against the sample
   values published in `cldr-core@<version>/supplemental/plurals.json`
   (`@integer` and `@decimal` lists), and
4. reports tags CLDR publishes that the table does not cover, and vice versa.

Usage:

    python3 verify_cldr_plurals.py --cldr-core /path/to/cldr-core/package
    python3 verify_cldr_plurals.py --cldr-core ... --quiet   # exit code only

Exit code 0 means the table matches CLDR on every representable sample.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[3]
DART_TABLE = (
    REPO_ROOT
    / "rustok_mobile/packages/rustok_i18n/lib/src/resolver/cldr_plurals.dart"
)

CASE_RE = re.compile(r"^    (?:case '(?P<tag>[A-Za-z-]+)'|(?P<default>default)):\s*$")
CATEGORY_ORDER = ("zero", "one", "two", "few", "many", "other")


class TableError(Exception):
    """The Dart table could not be read or is structurally invalid."""


# --------------------------------------------------------------------------- #
# Reading the Dart switch
# --------------------------------------------------------------------------- #

def _switch_bounds(lines: list[str]) -> tuple[int, int]:
    try:
        start = next(i for i, line in enumerate(lines) if line.strip() == 'switch (lang) {')
    except StopIteration as error:  # pragma: no cover - guarded by tests
        raise TableError("no `switch (lang) {` in the Dart table") from error
    for i in range(start + 1, len(lines)):
        if lines[i].strip() == '}' and lines[i - 1].strip().startswith('return'):
            return start, i
    raise TableError("the Dart switch is not closed by a returning body")


def _bodies(lines: list[str]) -> list[dict]:
    """Case groups of the switch: labels, body statements and line numbers."""
    start, end = _switch_bounds(lines)
    labels = [i for i in range(start, end) if CASE_RE.match(lines[i])]
    groups = []
    for position, index in enumerate(labels):
        stop = labels[position + 1] if position + 1 < len(labels) else end
        body = [
            (n, lines[n])
            for n in range(index + 1, stop)
            if lines[n].strip() and not lines[n].strip().startswith('//')
        ]
        match = CASE_RE.match(lines[index])
        groups.append(
            {
                "line": index + 1,
                "tag": match.group("tag"),
                "default": match.group("default") is not None,
                "body": body,
            }
        )
    if not groups:
        raise TableError("the Dart switch has no cases")
    return groups


def structural_problems(groups: list[dict]) -> list[str]:
    """Bodies that Dart would reject: an implicit fall-through or a missing return."""
    problems = []
    for group in groups:
        body = group["body"]
        if not body:
            continue  # stacked labels: an intentionally empty body
        last = body[-1][1].strip()
        if not last.startswith(("return ", "break", "continue", "throw")):
            problems.append(
                f"line {group['line']}: case '{group['tag']}' can complete "
                f"normally (last statement: {last!r})"
            )
    if not groups[-1]["default"]:
        problems.append("the switch has no trailing `default:` case")
    return problems


# --------------------------------------------------------------------------- #
# Translating the bodies to Python
# --------------------------------------------------------------------------- #

def _to_python(condition: str) -> str:
    text = condition
    text = text.replace("_inRange(", "_in_range(")
    text = text.replace("&&", " and ").replace("||", " or ")
    text = re.sub(r"!(?=[(\w])", " not ", text)
    # `int` division or bit operators do not appear in the table, so the text is
    # already valid Python once the operators above are rewritten.
    return text


IF_RE = re.compile(r"if \((?P<condition>.*?)\) return '(?P<category>\w+)';\s*", re.S)
RETURN_RE = re.compile(r"return '(?P<category>\w+)';\s*")


def _statements(group: dict) -> list[tuple[str, str | None]]:
    """Statements of one case body as (category, condition) pairs.

    Continuation lines of a wrapped `if (...)` are joined first, so a statement
    always ends at its semicolon.
    """
    text = " ".join(line.strip() for _, line in group["body"])
    statements = []
    while text.strip():
        match = IF_RE.match(text.strip())
        if match:
            statements.append((match.group("category"), match.group("condition")))
            text = text.strip()[match.end():]
            continue
        match = RETURN_RE.match(text.strip())
        if match:
            statements.append((match.group("category"), None))
            text = text.strip()[match.end():]
            continue
        raise TableError(f"unsupported statement in case '{group['tag']}': {text.strip()[:80]!r}")
    return statements


def _rule_source(groups: list[dict]) -> str:
    """Python source of `rule(lang, n, i, v, f, **operands)`."""
    chunks = []
    pending: list[str] = []
    for group in groups:
        if group["default"]:
            continue
        tags = pending + [group["tag"]]
        if not group["body"]:
            # Stacked labels: the next body belongs to these tags too.
            pending = tags
            continue
        pending = []
        body = []
        for category, condition in _statements(group):
            if condition is None:
                body.append(f"        return {category!r}")
            else:
                body.append(
                    f"        if {_to_python(condition)}:\n            return {category!r}"
                )
        for tag in tags:
            chunks.append("    if lang == {!r}:\n{}".format(tag, "\n".join(body)))
    signature = (
        "def rule(lang, n, i, v, f, n10, n100, n1000, n100000, n1000000, "
        "i10, i100, i1000000, f10, f100):\n"
    )
    return signature + "\n".join(chunks) + "\n    return 'other'\n"


def load_rule() -> tuple:
    """Returns (callable, groups) for the Dart table."""
    lines = DART_TABLE.read_text(encoding="utf-8").split("\n")
    groups = _bodies(lines)
    problems = structural_problems(groups)
    namespace: dict = {"_in_range": _in_range_exact}
    exec(_rule_source(groups), namespace)  # noqa: S102 - the source is the table itself
    return namespace["rule"], groups, problems


def _in_range_exact(value, low, high) -> bool:
    """Mirrors the Dart `_inRange`: integral values inside an inclusive range."""
    return value == int(value) and low <= value <= high


# --------------------------------------------------------------------------- #
# Operands and CLDR samples
# --------------------------------------------------------------------------- #

def operands(value: float) -> dict:
    """The CLDR operands for `value`, as the Dart table computes them."""
    n = abs(value)
    integral = n == int(n)
    text = repr(n).lower()
    if integral:
        v = f = 0
    else:
        mantissa, _, exponent_text = text.partition("e")
        exponent = int(exponent_text) if exponent_text else 0
        fraction_width = len(mantissa.split(".", 1)[1]) if "." in mantissa else 0
        v = max(0, fraction_width - exponent)
        digits = mantissa.replace(".", "")
        f = int(digits[-v:]) if v else 0
    i = int(n)
    return {
        "n": n,
        "i": i,
        "v": v,
        "f": f,
        "n10": n % 10,
        "n100": n % 100,
        "n1000": n % 1000,
        "n100000": n % 100000,
        "n1000000": n % 1000000,
        "i10": i % 10,
        "i100": i % 100,
        "i1000000": i % 1000000,
        "f10": f % 10,
        "f100": f % 100,
    }


SAMPLE_PART_RE = re.compile(r"^@(?P<kind>integer|decimal)\s+(?P<values>.*)$")


def _values_of(token: str) -> list[tuple[float, int]]:
    """Expands one CLDR sample token into (value, visible digits) pairs."""
    if "~" not in token:
        decimals = len(token.split(".")[1]) if "." in token else 0
        return [(float(token), decimals)]
    low_text, high_text = token.split("~", 1)
    decimals = max(
        len(part.split(".")[1]) if "." in part else 0
        for part in (low_text, high_text)
    )
    if decimals == 0:
        return [
            (float(value), 0) for value in range(int(low_text), int(high_text) + 1)
        ]
    step = 10.0 ** -decimals
    count = round((float(high_text) - float(low_text)) / step)
    return [
        (round(float(low_text) + index * step, decimals), decimals)
        for index in range(count + 1)
    ]


def samples(rule_text: str) -> list[tuple[float, int]]:
    """Sample values of one CLDR rule clause, with their written fraction digits.

    Ranges are expanded; compact notation (`1c6`) and the open-ended `…` tails
    are skipped because no Dart `num` represents them.
    """
    out: list[tuple[float, int]] = []
    for part in rule_text.split("@"):
        match = SAMPLE_PART_RE.match("@" + part.strip()) if part.strip() else None
        if not match:
            continue
        for token in match.group("values").split(","):
            token = token.strip()
            if not token or token.endswith("…") or "c" in token:
                continue
            out.extend(_values_of(token))
    return out


def cldr_samples(cldr_core: pathlib.Path) -> tuple[list, int]:
    """Samples of every CLDR rule, plus the count Dart cannot represent.

    A sample like `1.0` asks the rule to see one visible fraction digit, but a
    Dart `num` only stores the value; `1.0` is indistinguishable from `1` (the
    documented limitation of the table's operands). Such samples are counted and
    skipped rather than reported as mismatches.
    """
    path = cldr_core / "supplemental/plurals.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    cardinal = data["supplemental"]["plurals-type-cardinal"]
    out = []
    unrepresentable = 0
    for tag, rules in cardinal.items():
        for key, text in rules.items():
            category = key.replace("pluralRule-count-", "")
            for value, written_digits in samples(text):
                digits = operands(value)["v"] if written_digits else 0
                if digits != written_digits:
                    unrepresentable += 1
                    continue
                out.append((tag, value, category))
    return out, unrepresentable


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cldr-core", required=True, help="extracted cldr-core/package")
    parser.add_argument("--quiet", action="store_true", help="print only the summary")
    parser.add_argument("--limit", type=int, default=20, help="mismatches to print")
    args = parser.parse_args()

    cldr_core = pathlib.Path(args.cldr_core)
    rule, groups, problems = load_rule()

    tags = {group["tag"] for group in groups if group["tag"]}
    published = {
        tag
        for tag in json.loads(
            (cldr_core / "supplemental/plurals.json").read_text(encoding="utf-8")
        )["supplemental"]["plurals-type-cardinal"]
    }
    published_languages = {tag.split("-")[0] for tag in published} | {"pt-pt"}
    missing = sorted(tag for tag in published_languages if tag not in tags)
    extra = sorted(tag for tag in tags if tag not in published_languages)

    samples_by_tag, unrepresentable = cldr_samples(cldr_core)
    mismatches = []
    checked = 0
    for tag, value, expected in samples_by_tag:
        key = tag.lower() if tag == "pt-PT" else tag.split("-")[0].lower()
        if key not in tags:
            continue  # covered by `missing`/`extra` above
        checked += 1
        got = rule(key, **operands(value))
        if got != expected:
            mismatches.append((tag, value, expected, got))

    if not args.quiet:
        print(f"table cases: {len(groups)} ({len(tags)} tags + default)")
        if problems:
            for problem in problems:
                print(f"  STRUCTURAL: {problem}")
        if missing:
            print(f"  tags CLDR publishes but the table lacks: {missing}")
        if extra:
            print(f"  tags in the table that CLDR does not publish: {extra}")
        for tag, value, expected, got in mismatches[: args.limit]:
            print(f"  MISMATCH {tag} {value}: expected {expected}, got {got}")

    ok = not problems and not missing and not mismatches
    print(
        f"{checked} samples checked, {len(mismatches)} mismatches, "
        f"{len(problems)} structural problems, {len(missing)} missing tags, "
        f"{unrepresentable} samples skipped as unrepresentable by `num`"
    )
    print("OK" if ok else "FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
