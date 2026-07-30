import { HISTOGRAM_BUCKETS, METRICS } from './social-graph-privacy-shadow-evidence.mjs';

function decode(key) {
  return JSON.parse(key);
}

function labelString(labels) {
  return Object.entries(labels)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}="${String(value)}"`)
    .join(',');
}

function sample(name, labels, value) {
  const suffix = Object.keys(labels).length === 0 ? '' : `{${labelString(labels)}}`;
  return `${name}${suffix} ${value}`;
}

export function canonicalizeSnapshot(snapshot) {
  const lines = [sample(METRICS.collectorStarted, {}, snapshot.collectorStarted)];

  for (const [key, value] of [...snapshot.observations.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const [operation, outcome] = decode(key);
    lines.push(sample(METRICS.observations, { operation, outcome }, value));
  }
  for (const [key, value] of [...snapshot.failures.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const [operation, errorCode, retryable] = decode(key);
    lines.push(sample(METRICS.failures, {
      error_code: errorCode,
      operation,
      retryable: retryable ? 'true' : 'false',
    }, value));
  }
  for (const [key] of [...snapshot.observations.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const [operation, outcome] = decode(key);
    for (const le of HISTOGRAM_BUCKETS) {
      const bucketKey = JSON.stringify([operation, outcome, le]);
      lines.push(sample(`${METRICS.duration}_bucket`, { le, operation, outcome }, snapshot.durationBuckets.get(bucketKey)));
    }
    lines.push(sample(`${METRICS.duration}_sum`, { operation, outcome }, snapshot.durationSums.get(key)));
    lines.push(sample(`${METRICS.duration}_count`, { operation, outcome }, snapshot.durationCounts.get(key)));
    lines.push(sample(METRICS.lastObservation, { operation, outcome }, snapshot.lastObservations.get(key)));
  }
  return Buffer.from(`${lines.join('\n')}\n`, 'utf8');
}
