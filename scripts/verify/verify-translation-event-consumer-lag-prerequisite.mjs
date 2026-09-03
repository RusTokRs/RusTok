#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  centralPlan: 'docs/modules/translation-implementation-plan.md',
  localPlan: 'crates/rustok-translation/docs/implementation-plan.md',
  translationRoot: 'crates/rustok-translation/src/lib.rs',
  observability: 'crates/rustok-translation/src/observability.rs',
  workflow: 'crates/rustok-translation/src/workflow.rs',
  eventContract: 'crates/rustok-events/src/translation_workflow.rs',
  position: 'crates/rustok-iggy/src/position.rs',
  runtimeMetrics: 'crates/rustok-telemetry/src/runtime_consumer_metrics.rs',
  eventRuntime: 'apps/server/src/services/event_transport_factory.rs',
  evidence:
    'crates/rustok-translation/contracts/evidence/translation-event-consumer-lag-prerequisite-source.json',
  handoff:
    'crates/rustok-translation/docs/translation-event-consumer-lag-prerequisite.md',
};

const centralPlan = read(paths.centralPlan);
const localPlan = read(paths.localPlan);
const translationRoot = read(paths.translationRoot);
const observability = read(paths.observability);
const workflow = read(paths.workflow);
const eventContract = read(paths.eventContract);
const position = read(paths.position);
const runtimeMetrics = read(paths.runtimeMetrics);
const eventRuntime = read(paths.eventRuntime);
const evidence = JSON.parse(read(paths.evidence));
const handoff = read(paths.handoff);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const marker of [
  '- [ ] complete broker-backed Translation event-consumer lag evidence.',
  'opaque cursor for durable consumer-position lag.',
]) requireText(centralPlan, marker, `${paths.centralPlan}: gate must remain open`);

for (const marker of [
  'broker-backed event lag remains a',
  'runtime consumer/outbox concern that must use durable positions rather than',
  'event age or opaque cursor values.',
]) requireText(localPlan, marker, `${paths.localPlan}: durable-position boundary`);

for (const marker of [
  'rustok_translation_provider_checkpoint_age_seconds',
  'this is not cursor distance',
]) requireText(observability, marker, `${paths.observability}: non-broker metrics`);

for (const marker of [
  'use rustok_events::TranslationWorkflowEvent;',
  'use rustok_outbox::TransactionalEventBus;',
  '.publish_contract_in_tx(',
  'TranslationWorkflowEvent::JobCreated',
]) requireText(workflow, marker, `${paths.workflow}: transactional workflow events`);

for (const marker of [
  'pub enum TranslationWorkflowEvent {',
  '"translation.job.created"',
  'pub fn schema_version(&self) -> u16',
]) requireText(eventContract, marker, `${paths.eventContract}: sealed event contract`);

for (const marker of [
  'pub struct ConsumerPartitionPosition {',
  'pub acknowledged_offset: Option<u64>',
  'pub high_watermark: u64',
  'pub struct ConsumerPositionSnapshot {',
  'pub fn is_complete(&self) -> bool',
  'pub fn total_lag(&self) -> Option<u64>',
  'pub fn max_lag(&self) -> Option<u64>',
  'pub struct IggyConsumerPositionObserver {',
  'for partition in topic.partitions',
  '.get_consumer_offset(',
]) requireText(position, marker, `${paths.position}: broker position primitive`);

for (const marker of [
  '"rustok_runtime_consumer_lag"',
  '&["consumer", "aggregation"]',
  'pub fn record_position_snapshot(',
  'let complete = total_lag.is_some() && max_lag.is_some();',
]) requireText(runtimeMetrics, marker, `${paths.runtimeMetrics}: shared lag metrics`);

for (const marker of [
  'Durable inbound consumers must reuse the exact configured connector.',
  'ctx.shared_insert(Arc::clone(&iggy_transport));',
]) requireText(eventRuntime, marker, `${paths.eventRuntime}: connector ownership`);

for (const forbidden of [
  'IggyConsumerPositionObserver',
  'PersistentContractConsumerGroup',
  'record_position_snapshot',
]) forbidText(
  translationRoot,
  forbidden,
  `${paths.translationRoot}: Translation-specific durable consumer is not yet composed`,
);

if (evidence.schema_version !== 1) {
  failures.push(`${paths.evidence}: schema_version must be 1`);
}
if (evidence.status !== 'source_prerequisite_only') {
  failures.push(`${paths.evidence}: status must remain source_prerequisite_only`);
}

for (const [key, expected] of Object.entries({
  translation_workflow_events_published_transactionally: true,
  translation_specific_durable_inbound_consumer_present: false,
  translation_consumer_group_named: false,
  translation_consumer_topic_named: false,
  broker_position_observer_available: true,
  shared_runtime_lag_metrics_available: true,
  module_checkpoint_age_is_consumer_lag: false,
  opaque_provider_cursor_is_consumer_lag: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_facts.${key} must be ${expected}`);
  }
}

for (const key of [
  'reuse_configured_iggy_transport',
  'persistent_consumer_group',
  'explicit_topic',
  'all_partition_snapshot',
  'committed_group_offsets',
  'partition_high_watermark',
  'incomplete_snapshot_fails_closed',
  'publish_shared_position_metrics',
]) {
  if (evidence.required_runtime_contract?.[key] !== true) {
    failures.push(`${paths.evidence}: required_runtime_contract.${key} must be true`);
  }
}

if (JSON.stringify(evidence.required_runtime_contract?.lag_aggregations) !== JSON.stringify(['total', 'max'])) {
  failures.push(`${paths.evidence}: lag_aggregations must be exactly ["total", "max"]`);
}
if (!Array.isArray(evidence.runtime_evidence) || evidence.runtime_evidence.length !== 0) {
  failures.push(`${paths.evidence}: runtime_evidence must remain empty in this prerequisite slice`);
}
for (const key of [
  'runtime_consumer_proven',
  'external_iggy_proven',
  'consumer_lag_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const marker of [
  'Status: **source prerequisite only / runtime evidence open**',
  'main@51d2147bd920c7c580c0eee47f376035e8d8b77a',
  '`IggyConsumerPositionObserver`',
  '`rustok_runtime_consumer_lag{consumer, aggregation}`',
  'The central Phase 3 checkbox stays open.',
  'Do not add a no-op consumer only to manufacture a lag metric.',
]) requireText(handoff, marker, `${paths.handoff}: truthful handoff`);

if (failures.length > 0) {
  console.error('Translation event-consumer lag prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Translation event-consumer lag remains open until a real durable consumer exposes complete broker-backed positions',
);
