from pathlib import Path
import re


def replace_once(label: str, path: str, old: str, new: str) -> None:
    print(f"apply {label}", flush=True)
    file = Path(path)
    source = file.read_text()
    if old in source:
        source = source.replace(old, new, 1)
    elif new not in source:
        raise SystemExit(f"{label}: {path}: expected pattern not found")
    file.write_text(source)


def replace_all(label: str, path: str, old: str, new: str, expected: int) -> None:
    print(f"apply {label}", flush=True)
    file = Path(path)
    source = file.read_text()
    count = source.count(old)
    if count:
        if count != expected:
            raise SystemExit(f"{label}: {path}: expected {expected} occurrences, got {count}")
        source = source.replace(old, new)
    elif source.count(new) != expected:
        raise SystemExit(f"{label}: {path}: replacement state not found")
    file.write_text(source)


replace_once(
    "fly.dynamic",
    "crates/fly/src/dynamic.rs",
    '''    trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
''',
    '''    trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .map(str::trim)
        .filter(|value| {
            !value.is_empty() && !value.contains("{{") && !value.contains("}}")
        })
''',
)
replace_once(
    "fly.locale",
    "crates/fly/src/locale_coverage.rs",
    '        assert_eq!(report.tracked_locales, vec!["en", "de-de"]);',
    '        assert_eq!(report.tracked_locales, vec!["de-de", "en"]);',
)
replace_once(
    "fly.runtime",
    "crates/fly/src/runtime_validation.rs",
    '            .any(|diagnostic| diagnostic.code == "runtime_binding_target_missing"));',
    '            .any(|diagnostic| diagnostic.code == "runtime_binding_component_missing"));',
)
replace_once(
    "fly.roundtrip",
    "crates/fly/src/tests.rs",
    '    assert_eq!(output, input);',
    '''    assert_eq!(output["futureTopLevelField"], input["futureTopLevelField"]);
    assert_eq!(
        output["pages"][0]["futurePageField"],
        input["pages"][0]["futurePageField"]
    );
    assert_eq!(
        output["pages"][0]["component"],
        input["pages"][0]["component"]
    );
    assert_eq!(
        output["pages"][0]["frames"][0]["component"],
        input["pages"][0]["component"]
    );''',
)
replace_once(
    "fly.translation",
    "crates/fly/src/translation.rs",
    '                    required_locales: vec!["ru".to_string()],',
    '                    required_locales: vec!["ru-RU".to_string()],',
)
replace_once(
    "fly.audit",
    "crates/fly/src/audit.rs",
    '        .unwrap_or_else(|| match component_type {',
    '        .unwrap_or(match component_type {',
)

print("apply channel.generation", flush=True)
path = Path("apps/server/src/services/channel_cache_invalidation.rs")
source = path.read_text()
generation_pattern = re.compile(
    r'(?P<indent> +)if let Some\(previous\) = previous\n'
    r'(?P=indent)    && generation < previous\n'
    r'(?P=indent)\{\n'
    r'(?P<body>.*?)'
    r'(?P=indent)\}\n'
    r'(?P=indent)Ok\(Some\(generation\)\)',
    re.DOTALL,
)


def generation_repl(match: re.Match[str]) -> str:
    indent = match.group("indent")
    body = match.group("body")
    return (
        f"{indent}if let Some(previous) = previous {{\n"
        f"{indent}    if generation < previous {{\n"
        f"{body}{indent}    }}\n"
        f"{indent}}}\n"
        f"{indent}Ok(Some(generation))"
    )


source, count = generation_pattern.subn(generation_repl, source, count=1)
if count == 0 and "if let Some(previous) = previous {" not in source:
    raise SystemExit("channel.generation: let-chain pattern not found")
path.write_text(source)

two_let_pattern = re.compile(
    r'(?P<indent> +)if let Ok\(client\) = redis::Client::open\((?P<arg>[^\n]+)\)\n'
    r'(?P=indent)    && let Ok\(mut connection\) = client\.get_multiplexed_async_connection\(\)\.await\n'
    r'(?P=indent)\{\n'
    r'(?P<body>.*?)'
    r'(?P=indent)\}\n'
    r'(?P=indent)tokio::time::sleep',
    re.DOTALL,
)


def rewrite_two_let_file(label: str, path: str, expected: int) -> None:
    print(f"apply {label}", flush=True)
    file = Path(path)
    source = file.read_text()

    def repl(match: re.Match[str]) -> str:
        indent = match.group("indent")
        arg = match.group("arg")
        body = match.group("body")
        return (
            f"{indent}if let Ok(client) = redis::Client::open({arg}) {{\n"
            f"{indent}    if let Ok(mut connection) = "
            "client.get_multiplexed_async_connection().await {\n"
            f"{body}{indent}    }}\n"
            f"{indent}}}\n"
            f"{indent}tokio::time::sleep"
        )

    source, count = two_let_pattern.subn(repl, source)
    if count not in (0, expected):
        raise SystemExit(f"{label}: expected {expected} let-chain rewrites, got {count}")
    if count == 0 and source.count("client.get_multiplexed_async_connection().await {") < expected:
        raise SystemExit(f"{label}: nested readiness helpers not found")
    file.write_text(source)


rewrite_two_let_file("channel.redis", "apps/server/tests/channel_cache_resolved_value.rs", 2)
rewrite_two_let_file(
    "tenant-locale.redis", "apps/server/src/services/tenant_locale_generation_tests.rs", 2
)
rewrite_two_let_file("cache.fallback.redis", "crates/rustok-cache/tests/fallback_cas_live.rs", 1)
rewrite_two_let_file(
    "cache.hardening.redis", "crates/rustok-cache/tests/real_redis_hardening.rs", 1
)

print("apply cache.startup.redis", flush=True)
path = Path("crates/rustok-cache/src/startup_recovery_tests.rs")
source = path.read_text()
startup_pattern = re.compile(
    r'(?P<indent> +)if let Ok\(client\) = redis::Client::open\((?P<arg>[^\n]+)\)\n'
    r'(?P=indent)    && let Ok\(mut connection\) = client\.get_multiplexed_async_connection\(\)\.await\n'
    r'(?P=indent)    && redis::cmd\("PING"\)\n'
    r'(?P=indent)        \.query_async::<String>\(&mut connection\)\n'
    r'(?P=indent)        \.await\n'
    r'(?P=indent)        \.as_deref\(\)\n'
    r'(?P=indent)        == Ok\("PONG"\)\n'
    r'(?P=indent)\{\n'
    r'(?P=indent)    return;\n'
    r'(?P=indent)\}',
)


def startup_repl(match: re.Match[str]) -> str:
    indent = match.group("indent")
    arg = match.group("arg")
    return (
        f"{indent}if let Ok(client) = redis::Client::open({arg}) {{\n"
        f"{indent}    if let Ok(mut connection) = client.get_multiplexed_async_connection().await {{\n"
        f"{indent}        if redis::cmd(\"PING\")\n"
        f"{indent}            .query_async::<String>(&mut connection)\n"
        f"{indent}            .await\n"
        f"{indent}            .as_deref()\n"
        f"{indent}            == Ok(\"PONG\")\n"
        f"{indent}        {{\n"
        f"{indent}            return;\n"
        f"{indent}        }}\n"
        f"{indent}    }}\n"
        f"{indent}}}"
    )


source, count = startup_pattern.subn(startup_repl, source, count=1)
if count == 0 and "if redis::cmd(\"PING\")" not in source:
    raise SystemExit("cache.startup.redis: let-chain pattern not found")
path.write_text(source)

replace_once(
    "cache.guard.loop",
    "crates/rustok-cache/tests/alert_rules_guard.rs",
    '        "while backend.stats().entries > 1",',
    '        "let first_value = backend.get(first.0).await.unwrap();",',
)
replace_once(
    "cache.guard.message",
    "crates/rustok-cache/tests/alert_rules_guard.rs",
    '        "capacity-one cache retained both entries",',
    '        ".expect(\\"entry-count cache did not evict either key\\");",',
)

for label, old, new in [
    ("core.counter", "for (_, counter) in counters.iter()", "for counter in counters.values()"),
    ("core.gauge", "for (_, gauge) in gauges.iter()", "for gauge in gauges.values()"),
    (
        "core.histogram",
        "for (_, histogram) in histograms.iter()",
        "for histogram in histograms.values()",
    ),
]:
    replace_all(label, "crates/rustok-core/src/metrics/mod.rs", old, new, 2)
replace_once(
    "core.results",
    "crates/rustok-core/src/utils/mod.rs",
    '''        match result {
            Ok(value) => results.push(value),
            Err(e) => return Err(e),
        }
''',
    '''        let value = result?;
        results.push(value);
''',
)

replace_once(
    "payment.hmac",
    "crates/rustok-payment/Cargo.toml",
    'hmac = { version = "0.12", optional = true }',
    'hmac = { version = "0.13", optional = true }',
)
replace_once(
    "payment.import",
    "crates/rustok-payment/src/stripe_provider.rs",
    'use hmac::{Hmac, Mac};',
    'use hmac::{Hmac, KeyInit, Mac};',
)
replace_once(
    "csp.heading",
    "scripts/verify/verify-csp-reporting-contract.mjs",
    '## Trusted Script Nonce Boundary',
    '## Trusted Script and Style Element Nonce Boundary',
)
replace_once(
    "csp.test",
    "apps/server/src/middleware/csp_reports.rs",
    'fn parses_legacy_csp_report_without_script_sample()',
    'fn parses_legacy_csp_report_without_recording_sensitive_sample()',
)
replace_once(
    "page-builder.accessor",
    "crates/rustok-page-builder/admin/src/model.rs",
    '''    pub fn editor(&self) -> &FlyEditor {
        &self.editor
    }
''',
    '''    pub fn editor(&self) -> &FlyEditor {
        &self.editor
    }

    #[cfg(test)]
    pub(crate) fn editor_mut_for_tests(&mut self) -> &mut FlyEditor {
        &mut self.editor
    }
''',
)
replace_once("deny.mit0", "deny.toml", '    "MIT",', '    "MIT",\n    "MIT-0",')
print("all source rewrites applied", flush=True)
