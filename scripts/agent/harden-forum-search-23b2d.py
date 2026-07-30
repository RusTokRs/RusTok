import json
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}: {old[:120]!r}")
    target.write_text(text.replace(old, new, 1))


replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "seen: &mut HashSet<(String, String, Uuid)>,",
    "seen: &mut HashSet<(String, String, Uuid, Option<String>)>,",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """            item.entity_type.clone(),
            item.id,
        )) {""",
    """            item.entity_type.clone(),
            item.id,
            item.locale.clone(),
        )) {""",
)

replace_once(
    "apps/server/src/services/forum_search_result_eligibility.rs",
    """        let locale = request.locale.trim();
        if locale.is_empty() {
            return Err(PortError::validation(
                "forum.search_result_eligibility.locale_required",
                "Forum Search result eligibility requires a locale",
            ));
        }
        let forum_candidates = request
""",
    """        let locale = request.locale.trim();
        if locale.is_empty() {
            return Err(PortError::validation(
                "forum.search_result_eligibility.locale_required",
                "Forum Search result eligibility requires a locale",
            ));
        }
        if request
            .auth
            .as_ref()
            .is_some_and(|auth| auth.tenant_id != request.tenant_id)
        {
            return Err(PortError::validation(
                "forum.search_result_eligibility.auth_tenant_mismatch",
                "Forum Search result eligibility auth tenant does not match the request",
            ));
        }
        if let Some(context) = request.request_context.as_ref() {
            if context.tenant_id != request.tenant_id {
                return Err(PortError::validation(
                    "forum.search_result_eligibility.request_tenant_mismatch",
                    "Forum Search result eligibility request tenant does not match the request",
                ));
            }
            if let Some(auth) = request.auth.as_ref()
                && context.user_id != Some(auth.user_id)
            {
                return Err(PortError::validation(
                    "forum.search_result_eligibility.request_actor_mismatch",
                    "Forum Search result eligibility request actor does not match auth",
                ));
            }
        }
        let forum_candidates = request
""",
)

contract_path = Path("crates/rustok-forum/contracts/forum-search-result-eligibility.json")
contract = json.loads(contract_path.read_text())
contract["bounds"]["raw_row_identity_includes_locale"] = True
contract["transport_authority"]["auth_tenant_must_match"] = True
contract["transport_authority"]["request_context_tenant_must_match"] = True
contract["transport_authority"]["request_context_actor_must_match_auth"] = True
contract_path.write_text(json.dumps(contract, indent=2) + "\n")

replace_once(
    "scripts/verify/verify-forum-search-result-eligibility.mjs",
    """    "candidate snapshot changed during bounded eligibility evaluation",
    "resolve_storefront_search_result_candidates",
""",
    """    "candidate snapshot changed during bounded eligibility evaluation",
    "item.locale.clone()",
    "candidate scan returned a duplicate raw row",
    "resolve_storefront_search_result_candidates",
""",
)
replace_once(
    "scripts/verify/verify-forum-search-result-eligibility.mjs",
    """    "Forum Search result eligibility is unavailable",
  ],
""",
    """    "Forum Search result eligibility is unavailable",
    "auth_tenant_mismatch",
    "request_tenant_mismatch",
    "request_actor_mismatch",
  ],
""",
)
replace_once(
    "scripts/verify/verify-forum-search-result-eligibility.mjs",
    """  if (contract.bounds?.raw_search_page_size !== 50) {
    failures.push(`${paths.contract}: raw page-size drift`);
  }
""",
    """  if (contract.bounds?.raw_search_page_size !== 50) {
    failures.push(`${paths.contract}: raw page-size drift`);
  }
  if (contract.bounds?.raw_row_identity_includes_locale !== true) {
    failures.push(`${paths.contract}: raw row locale identity drift`);
  }
  for (const key of [
    "auth_tenant_must_match",
    "request_context_tenant_must_match",
    "request_context_actor_must_match_auth",
  ]) {
    if (contract.transport_authority?.[key] !== true) {
      failures.push(`${paths.contract}: transport_authority ${key} drift`);
    }
  }
""",
)
