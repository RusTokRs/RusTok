import { maskRustNonCode } from './native-module-layout.mjs';

function normalizeRust(source) {
  return maskRustNonCode(source).replace(/\s+/g, ' ').replace(/\s*(::|[.=(),;])\s*/g, '$1');
}

function outboxBusFor(connection) {
  return String.raw`\blet (?<bus>[A-Za-z_]\w*)=TransactionalEventBus::new\(Arc::new\(OutboxTransport::new\(${connection}\)\)\);`;
}

function restartHandlerWiring(connection, handler, prefix = '') {
  const construction = String.raw`BlogCommentProjectionHandler::new\(${connection},\k<bus>(?:\.clone\(\))?,?\);`;
  return new RegExp(`${prefix}${outboxBusFor(connection)}\\s*let ${handler}=${construction}`);
}

const moduleConnection = String.raw`ctx\.db\.clone\(\)`;
const listenerRegistration = new RegExp([
  String.raw`\bfn register_event_listeners\([^)]*\)\s*\{[^{}]*?`,
  outboxBusFor(moduleConnection),
  String.raw`[^{}]*?\bregistry\.register\(services::BlogCommentProjectionHandler::new\(`,
  String.raw`${moduleConnection},\k<bus>\.clone\(\),?\),?\);`,
].join(''));
const firstHandler = restartHandlerWiring(String.raw`test_db\.db\.clone\(\)`, 'first_handler');
const restartedHandler = restartHandlerWiring(
  String.raw`restarted_db\.clone\(\)`,
  'restarted_handler',
  String.raw`\blet restarted_db=test_db\.restarted_connection\(\)\.await\?;\s*`,
);
const replayHandler = new RegExp([
  outboxBusFor(String.raw`replay_db\.clone\(\)`),
  String.raw`\s*BlogCommentProjectionHandler::new\(replay_db,\k<bus>,?\)\.handle\(&envelope\)`,
].join(''));

export function hasBlogProjectionRegistration(source) {
  return listenerRegistration.test(normalizeRust(source));
}

export function hasBlogProjectionReplay(source) {
  return replayHandler.test(normalizeRust(source));
}

export function blogProjectionRestartWiringFailures(source) {
  const code = normalizeRust(source);
  const failures = [];
  if (!firstHandler.test(code)) {
    failures.push('first handler must receive an outbox bus built from the initial database connection');
  }
  if (!restartedHandler.test(code)) {
    failures.push('restarted handler must receive a fresh outbox bus built from the restarted database connection');
  }
  return failures;
}
