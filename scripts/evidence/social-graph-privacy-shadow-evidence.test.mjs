#!/usr/bin/env node

import assert from 'node:assert/strict';
import test from 'node:test';

import {
  HISTOGRAM_BUCKETS,
  METRICS,
  analyzeWindow,
  assessMetrics,
  parseSnapshot,
} from './lib/social-graph-privacy-shadow-evidence.mjs';
import { canonicalizeSnapshot } from './lib/social-graph-privacy-shadow-canonical.mjs';

const WINDOW = {
  started_at: '2026-07-30T08:00:00.000Z',
  ended_at: '2026-07-30T08:10:00.000Z',
  duration_seconds: 600,
};

function labelString(labels) {
  return Object.entries(labels)
    .map(([key, value]) => `${key}="${value}"`)
    .join(',');
}

function sample(name, labels, value) {
  const suffix = Object.keys(labels).length === 0 ? '' : `{${labelString(labels)}}`;
  return `${name}${suffix} ${value}`;
}

function series(operation, outcome, count, sum, timestamp) {
  const labels = { operation, outcome };
  const lines = [sample(METRICS.observations, labels, count)];
  for (const le of HISTOGRAM_BUCKETS) {
    lines.push(sample(`${METRICS.duration}_bucket`, { operation, outcome, le }, count));
  }
  lines.push(sample(`${METRICS.duration}_sum`, labels, sum));
  lines.push(sample(`${METRICS.duration}_count`, labels, count));
  lines.push(sample(METRICS.lastObservation, labels, timestamp));
  return lines;
}

function snapshot({ epoch = 1_700_000_000, entries = [], extra = [] } = {}) {
  return Buffer.from([
    sample(METRICS.collectorStarted, {}, epoch),
    ...entries.flatMap((entry) => series(...entry)),
    ...extra,
    '',
  ].join('\n'));
}

function notificationCoverageEntries() {
  return [
    ['blocks_between', 'match_positive', 25, 0.25, 1_785_398_450],
    ['blocks_between', 'match_negative', 25, 0.25, 1_785_398_500],
    ['source_mutes_target', 'match_positive', 25, 0.25, 1_785_398_550],
    ['source_mutes_target', 'match_negative', 25, 0.25, 1_785_398_600],
  ];
}

test('canonical retained snapshot excludes unrelated process metrics', () => {
  const parsed = parseSnapshot(snapshot({
    entries: [['blocks_between', 'match_positive', 1, 0.01, 1_785_398_450]],
    extra: ['http_requests_total{path="/private/tenant"} 99'],
  }));
  const retained = canonicalizeSnapshot(parsed).toString('utf8');
  assert.match(retained, /rustok_social_graph_index_privacy_shadow_observations_total/);
  assert.doesNotMatch(retained, /http_requests_total/);
  assert.doesNotMatch(retained, /private\/tenant/);
});

test('collector epoch change rejects a restarted observation window', () => {
  const start = parseSnapshot(snapshot({ epoch: 100 }));
  const end = parseSnapshot(snapshot({
    epoch: 101,
    entries: [['blocks_between', 'match_positive', 1, 0.01, 1_785_398_450]],
  }));
  assert.throws(() => analyzeWindow(start, end, WINDOW), /collector epoch changed/);
});

test('counter decrease rejects a reset inside one collector epoch', () => {
  const start = parseSnapshot(snapshot({
    entries: [['blocks_between', 'match_positive', 2, 0.02, 1_785_398_410]],
  }));
  const end = parseSnapshot(snapshot({
    entries: [['blocks_between', 'match_positive', 1, 0.01, 1_785_398_450]],
  }));
  assert.throws(() => analyzeWindow(start, end, WINDOW), /counter reset detected/);
});

test('unknown labels under the shadow prefix fail closed', () => {
  const bytes = Buffer.from([
    sample(METRICS.collectorStarted, {}, 100),
    sample(METRICS.observations, {
      operation: 'blocks_between',
      outcome: 'match_positive',
      tenant_id: 'secret',
    }, 1),
    '',
  ].join('\n'));
  assert.throws(() => parseSnapshot(bytes), /labels .* must be exactly/);
});

test('false negative evidence is reviewable but cannot pass policy', () => {
  const start = parseSnapshot(snapshot());
  const end = parseSnapshot(snapshot({
    entries: [
      ...notificationCoverageEntries(),
      ['blocks_between', 'false_negative', 1, 0.01, 1_785_398_650],
    ],
  }));
  const metrics = analyzeWindow(start, end, WINDOW);
  const assessment = assessMetrics(metrics, {
    minimum_observations: 100,
    maximum_error_rate_basis_points: 0,
    maximum_p95_seconds: 0.1,
    require_notification_positive_and_negative_coverage: true,
    require_zero_mismatches: true,
  });
  assert.equal(metrics.totals.false_negative, 1);
  assert.equal(assessment.zero_negative_safety_misses, false);
  assert.equal(assessment.policy_passed, false);
});
