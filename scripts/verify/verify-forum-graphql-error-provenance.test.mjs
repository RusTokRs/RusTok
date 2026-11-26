import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { DOMAIN_ERROR_REGIONS, inspectDomainErrors } from "./verify-forum-graphql-error-provenance.mjs";

for (const region of DOMAIN_ERROR_REGIONS) {
  const source = readFileSync(new URL(`../../crates/modules/rustok-forum/src/graphql/${region.file}`, import.meta.url), "utf8");
  test(`${region.file} preserves typed domain errors and existing absence behavior`, () => {
    assert.deepEqual(inspectDomainErrors(source, region), []);
  });
  const start = source.indexOf(region.start);
  const end = source.indexOf(region.end, start);
  const body = source.slice(start, end);
  for (const [index, match] of [...body.matchAll(/Err\(error\) => (?:return )?Err\(error\.into\(\)\)/g)].entries()) {
    test(`${region.file} rejects string erasure in error arm ${index}`, () => {
      const at = start + match.index;
      const changed = match[0].replace("error.into()", "async_graphql::Error::new(error.to_string())");
      const mutated = source.slice(0, at) + changed + source.slice(at + match[0].length);
      assert.notDeepEqual(inspectDomainErrors(mutated, region), []);
    });
  }
  for (const marker of [".can_disclose_public_gone(", ".compose_admitted_profiles(", ".read_for_audience("]) {
    const call = body.indexOf(marker);
    if (call < 0) continue;
    test(`${region.file} rejects source erasure after ${marker}`, () => {
      const at = source.indexOf(".await?", start + call);
      assert.ok(at > start && at < end);
      const mutated = source.slice(0, at) + '.await.map_err(|error| async_graphql::Error::new(error.to_string()))?' + source.slice(at + ".await?".length);
      assert.notDeepEqual(inspectDomainErrors(mutated, region), []);
    });
  }
  if (region.file === "member_card_query.rs") {
    test("member-card validation cannot erase a typed error", () => {
      const mutated = source.replace(/normalize_user_ids\(&user_ids\)\s*\?/, 'normalize_user_ids(&user_ids).map_err(|error| async_graphql::Error::new(error.to_string()))?');
      assert.notEqual(mutated, source);
      assert.notDeepEqual(inspectDomainErrors(mutated, region), []);
    });
  } else {
    test(`${region.file} cannot convert absence into a disclosure/error`, () => {
      const branch = region.file === "query_runtime.rs"
        ? "Err(ForumError::CategoryNotFound(_)) => return Ok(None)"
        : region.file === "category_route_query.rs"
          ? "Err(ForumError::CategoryRouteNotFound) => Ok(None)"
          : "Err(ForumError::TopicRouteNotFound) => return Ok(None)";
      assert.ok(source.includes(branch));
      const mutated = source.replace(branch, branch.replace("Ok(None)", "Err(ForumError::TopicRouteNotFound.into())"));
      assert.notDeepEqual(inspectDomainErrors(mutated, region), []);
    });
  }
}

const categoryRegion = DOMAIN_ERROR_REGIONS.find(({ file }) => file === "category_route_query.rs");
const categorySource = readFileSync(new URL("../../crates/modules/rustok-forum/src/graphql/category_route_query.rs", import.meta.url), "utf8");
test("category route audience context cannot replace validation with an internal error", () => {
  const mutated = categorySource.replace(/(let audience_context = category_read_audience_port_context\([^;]*?\))\?;/,
    '$1.map_err(|error| internal_error(error.to_string()))?;');
  assert.notEqual(mutated, categorySource);
  assert.notDeepEqual(inspectDomainErrors(mutated, categoryRegion), []);
});
for (const [index, match] of [...categorySource.matchAll(/Err\(error\) => (?:return )?Err\(error\.into\(\)\)/g)].entries()) {
  test(`category route rejects its original internal-error wrapper in branch ${index}`, () => {
    const mutated = categorySource.slice(0, match.index) + match[0].replace("error.into()", "internal_error(error.to_string())") + categorySource.slice(match.index + match[0].length);
    assert.notDeepEqual(inspectDomainErrors(mutated, categoryRegion), []);
  });
}
