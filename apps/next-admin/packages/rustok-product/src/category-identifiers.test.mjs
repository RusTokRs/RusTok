import test from 'node:test';
import assert from 'node:assert/strict';
import {
  categoryCodeFromName,
  slugifyCategoryName
} from './category-identifiers.ts';

function utf8Length(value) {
  return Array.from(value).reduce((total, character) => {
    const point = character.codePointAt(0) ?? 0;
    return (
      total +
      (point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4)
    );
  }, 0);
}

test('category slug generation matches the Product Unicode slug contract', () => {
  assert.equal(slugifyCategoryName('Красный чай 2026'), 'красный-чай-2026');
  assert.equal(slugifyCategoryName('東京 2026'), '東京-2026');
  assert.equal(slugifyCategoryName('Café Déjà Vu'), 'café-déjà-vu');
  assert.equal(slugifyCategoryName('API'), 'api-1');
  assert.equal(slugifyCategoryName('***'), 'untitled');
});

test('category slugs remain canonical and within the Taxonomy route byte limit', () => {
  const slug = slugifyCategoryName('東京'.repeat(80));
  assert.ok(utf8Length(slug) <= 120);
  assert.equal(slugifyCategoryName(slug), slug);
});

test('category codes are valid and bounded for ASCII and Unicode names', () => {
  assert.equal(
    categoryCodeFromName('Mechanical Keyboards'),
    'mechanical_keyboards'
  );
  assert.match(categoryCodeFromName('Красный чай'), /^u_[a-f0-9_]+$/);
  assert.ok(categoryCodeFromName('分類'.repeat(100)).length <= 128);
  assert.match(categoryCodeFromName('分類'.repeat(100)), /^[a-z0-9_-]+$/);
});
