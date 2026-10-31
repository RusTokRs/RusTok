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

test('formatReadingTime formats English reading times', () => {
  assert.equal(formatReadingTime(1, 'en'), '1 min read');
  assert.equal(formatReadingTime(5, 'en'), '5 min read');
  assert.equal(formatReadingTime(0, 'en'), '1 min read');
  assert.equal(formatReadingTime(-3, 'en'), '1 min read');
});

test('formatReadingTime handles Russian pluralization correctly across boundaries', () => {
  assert.equal(formatReadingTime(1, 'ru'), '1 минута чтения');
  assert.equal(formatReadingTime(2, 'ru'), '2 минуты чтения');
  assert.equal(formatReadingTime(3, 'ru'), '3 минуты чтения');
  assert.equal(formatReadingTime(4, 'ru'), '4 минуты чтения');
  assert.equal(formatReadingTime(5, 'ru'), '5 минут чтения');
  assert.equal(formatReadingTime(11, 'ru'), '11 минут чтения');
  assert.equal(formatReadingTime(12, 'ru'), '12 минут чтения');
  assert.equal(formatReadingTime(14, 'ru'), '14 минут чтения');
  assert.equal(formatReadingTime(20, 'ru'), '20 минут чтения');
  assert.equal(formatReadingTime(21, 'ru'), '21 минута чтения');
  assert.equal(formatReadingTime(22, 'ru'), '22 минуты чтения');
  assert.equal(formatReadingTime(24, 'ru'), '24 минуты чтения');
  assert.equal(formatReadingTime(25, 'ru'), '25 минут чтения');
  assert.equal(formatReadingTime(101, 'ru'), '101 минута чтения');
  assert.equal(formatReadingTime(111, 'ru'), '111 минут чтения');
});
