// GitHub Actions references in workflow sources.
//
// Repository policy is "every third-party action is pinned to an immutable commit SHA", and it is
// enforced once, by `scripts/verify/verify-workflow-action-pins.mjs`, which runs in CI on every
// workflow file. Guards that additionally hard-coded one particular commit duplicated that policy
// with a value that goes stale on every dependency bump: the bump moves the pin, the literal stays,
// and the gate reports a correctly pinned workflow as broken — while still not catching a missing
// action, which is the rule those guards actually wanted.
//
// These helpers check the shape the policy cares about: the action is present and is referenced by a
// 40-character commit SHA, never by a mutable tag or branch.

const escape = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Every SHA-pinned reference to `action`, e.g. `actions/upload-artifact@<40 hex>`. */
export function pinnedActionRefs(workflow, action) {
  return [...workflow.matchAll(new RegExp(`${escape(action)}@[0-9a-f]{40}`, "g"))].map((match) => match[0]);
}

export function hasPinnedAction(workflow, action) {
  return pinnedActionRefs(workflow, action).length > 0;
}

export function countPinnedAction(workflow, action) {
  return pinnedActionRefs(workflow, action).length;
}

/** True when `action` is referenced by anything other than a 40-character commit SHA. */
export function hasMutableActionRef(workflow, action) {
  return new RegExp(`${escape(action)}@(?![0-9a-f]{40})[^\\s#]+`).test(workflow);
}

/**
 * One message describing what is wrong with `action` in this workflow, or `null` when it is used and
 * pinned. Callers report it in their own idiom (`fail(...)` or `failures.push(...)`).
 */
export function pinnedActionProblem(workflow, action) {
  if (hasMutableActionRef(workflow, action)) {
    return `${action} must be referenced by an immutable commit SHA, not a mutable tag`;
  }
  if (!hasPinnedAction(workflow, action)) return `${action} is not used`;
  return null;
}
