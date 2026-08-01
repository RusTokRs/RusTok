import fs from 'node:fs';

const evidence = JSON.parse(fs.readFileSync('crates/rustok-blog/contracts/evidence/blog-comments-tcp-server-adapter.json', 'utf8'));
const server = fs.readFileSync('crates/rustok-comments/src/tcp_server.rs', 'utf8');
const delegation = fs.readFileSync('crates/rustok-comments/src/tcp_delegation.rs', 'utf8');
const plan = fs.readFileSync('crates/rustok-blog/docs/implementation-plan-slice-69.md', 'utf8');

function requireCondition(condition, message) {
  if (!condition) throw new Error(message);
}
function requireText(source, fragment, label) {
  requireCondition(source.includes(fragment), `${label} missing ${fragment}`);
}

requireCondition(evidence.schema_version === 1, 'evidence schema drift');
requireCondition(evidence.surface === 'comments_tcp_server_adapter', 'evidence identity drift');
requireCondition(evidence.status === 'source_verified_no_compile', 'historical status drift');
requireCondition(evidence.compile_policy === 'not_run_by_request', 'compile policy drift');
requireCondition(evidence.runtime_status === 'not_run', 'runtime status drift');

for (const fragment of [
  'pub const ALL: [Self; 7]',
  'pub const fn as_str(self)',
  'pub const fn is_write(self) -> bool',
  'pub fn for_request(request: &CommentsThreadRequest)',
  'pub trait CommentsTcpAuthorityResolver: Send + Sync',
  'request: &CommentsThreadRequest',
  'let operation = CommentsTcpOperation::for_request(&request);',
  '.authorize(peer_addr, operation, credential.as_ref(), &request)',
  'apply_authority(request.context(), authority)?',
  'replace_request_context(&mut request, trusted_context);',
  'dispatch_request(self.provider.as_ref(), request).await',
]) requireText(server, fragment, 'server adapter');
requireText(delegation, 'CommentsTcpOperation::for_request(request) != operation', 'delegation binding');
requireCondition(!server.includes('TcpListener'), 'adapter must not own listener');
requireCondition(!server.includes('struct AllowAll'), 'adapter must not allow all authority');
requireText(plan, '# rustok-blog implementation plan — slice 69 continuation', 'slice-69 plan');
requireText(plan, 'source_verified_no_compile', 'slice-69 plan');

console.log('[verify-blog-comments-tcp-server-adapter-retained] retained contract verified');
