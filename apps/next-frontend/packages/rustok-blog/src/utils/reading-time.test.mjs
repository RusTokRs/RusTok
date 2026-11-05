import assert from 'node:assert/strict';
import { test } from 'node:test';
import { calculateReadingTime, formatReadingTime } from './reading-time.ts';

test('calculateReadingTime handles empty and short texts with minimum 1 min', () => {
  assert.equal(calculateReadingTime(''), 1);
  assert.equal(calculateReadingTime('   '), 1);
  assert.equal(calculateReadingTime('Hello world'), 1);
});

test('calculateReadingTime calculates correct minutes for larger word counts', () => {
  const words199 = new Array(199).fill('word').join(' ');
  const words200 = new Array(200).fill('word').join(' ');
  const words201 = new Array(201).fill('word').join(' ');

  assert.equal(calculateReadingTime(words199), 1);
  assert.equal(calculateReadingTime(words200), 1);
  assert.equal(calculateReadingTime(words201), 2);
});

test('reading-time copy uses real Fluent catalogs and plural rules', async () => {
  const { createFluentBundle, createTranslator } = await import('@rustok/next-fluent');
  const { readFile } = await import('node:fs/promises');
  const translator = async locale => createTranslator(createFluentBundle(locale,
    await readFile(new URL(`../../../../messages/${locale}.ftl`, import.meta.url), 'utf8')), { namespace: 'Blog' });
  const visible = text => text.replace(/[\u2068\u2069]/g, '');
  const en = await translator('en'); const ru = await translator('ru');
  assert.equal(visible(formatReadingTime(1, en)), '1 min read');
  for (const [minutes, expected] of [[1, '1 минута чтения'], [2, '2 минуты чтения'], [5, '5 минут чтения'], [11, '11 минут чтения'], [21, '21 минута чтения']]) {
    assert.equal(visible(formatReadingTime(minutes, ru)), expected);
  }
  const fr = createTranslator(createFluentBundle('fr', `Blog-readingTime = { $minutes ->
    [one] { $minutes } minute de lecture
   *[other] { $minutes } minutes de lecture
}`), { namespace: 'Blog' });
  assert.equal(visible(formatReadingTime(1, fr)), '1 minute de lecture');
  assert.equal(visible(formatReadingTime(3, fr)), '3 minutes de lecture');
});

test('reading time bounds non-finite inputs and invalid rates', () => {
  assert.equal(calculateReadingTime('word '.repeat(201), 0), 2);
  assert.equal(calculateReadingTime('word '.repeat(201), NaN), 2);
  const t = (_key, { minutes }) => String(minutes);
  for (const input of [NaN, Infinity, -3, 0]) assert.equal(formatReadingTime(input, t), '1');
});
