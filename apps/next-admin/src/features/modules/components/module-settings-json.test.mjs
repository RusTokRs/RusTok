import assert from 'node:assert/strict';
import test from 'node:test';
import {
  formatSettingsJson,
  mergeSettingsJson,
  parseSettingsObject,
  parseSettingsUnsignedInteger
} from './module-settings-json.ts';

test('display, edit and submission preserve large nested numbers', () => {
  const text =
    '{"quota":9007199254740993,"nested":[18446744073709551615,1.2300],"enabled":false}';
  const formatted = formatSettingsJson(text, true);
  const edited = mergeSettingsJson(formatted, { enabled: true });
  assert.equal(
    formatSettingsJson(edited, false),
    '{"quota":9007199254740993,"nested":[18446744073709551615,1.2300],"enabled":true}'
  );
});

test('numeric field edits retain exact integer text, including zero', () => {
  const text = '{"limit":9007199254740993,"other":18446744073709551615}';
  assert.equal(parseSettingsObject(text).limit.toString(), '9007199254740993');
  const edited = mergeSettingsJson(text, {
    limit: parseSettingsUnsignedInteger('18446744073709551615')
  });
  assert.equal(
    formatSettingsJson(edited, false),
    '{"limit":18446744073709551615,"other":18446744073709551615}'
  );
  assert.equal(
    formatSettingsJson(
      mergeSettingsJson(text, { limit: parseSettingsUnsignedInteger('0') }),
      false
    ),
    '{"limit":0,"other":18446744073709551615}'
  );
  for (const invalid of ['', '-', '-1', '01', '1.5', '1e3', ' 1', 'NaN']) {
    assert.throws(() => parseSettingsUnsignedInteger(invalid));
  }
});

test('invalid and non-object settings cannot be silently merged', () => {
  for (const text of ['{', '[]', 'null', '9007199254740993']) {
    assert.throws(() => mergeSettingsJson(text, { enabled: true }));
  }
  assert.throws(() => formatSettingsJson('{"a":1,"a":2}', false));
});
