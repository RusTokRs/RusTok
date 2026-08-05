#!/usr/bin/env node

import fs from 'node:fs';

const read = (relative) => fs.readFileSync(relative, 'utf8');
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`${label}: missing ${JSON.stringify(needle)}`);
  }
};
const rejectText = (source, needle, label) => {
  if (source.includes(needle)) {
    throw new Error(`${label}: forbidden ${JSON.stringify(needle)}`);
  }
};

const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-reply-range-move-admin-ui.json')
);
if (contract.contract !== 'forum_reply_range_move_admin_ui_v1') {
  throw new Error('unexpected contract id');
}
if (contract.task !== 'FORUM-21X') {
  throw new Error('contract task must be FORUM-21X');
}
if (contract.command.graphql_field !== 'moveForumTopicReplyRange') {
  throw new Error('contract must retain the owner GraphQL field');
}
if (contract.composition.transport_fallback !== false) {
  throw new Error('transport fallback must remain disabled');
}
if (
  contract.position_preflight.transport_infers_positions_from_row_order !== false
) {
  throw new Error('UI must not infer canonical positions from row order');
}

const nextModel = read(
  'apps/next-admin/packages/forum/src/core/topic-reply-range.ts'
);
requireText(nextModel, 'buildForumReplyRangeMoveCommand', 'Next model');
requireText(nextModel, 'input.startPosition > input.endPosition', 'Next range order');
requireText(nextModel, 'newForumReplyRangeMoveIdentity', 'Next retry identity');

const nextApi = read(
  'apps/next-admin/packages/forum/src/api/topic-reply-range.ts'
);
requireText(nextApi, 'moveForumTopicReplyRange', 'Next GraphQL adapter');
requireText(
  nextApi,
  'MoveForumTopicReplyRangeGraphqlInput',
  'Next GraphQL input'
);
rejectText(nextApi, 'forum_reply_range_move_operations', 'Next audit boundary');
rejectText(nextApi, '/api/forum', 'Next REST fallback');

const nextUi = read(
  'apps/next-admin/packages/forum/src/components/forum-topic-reply-range.tsx'
);
requireText(nextUi, "type='number'", 'Next position inputs');
requireText(nextUi, 'commandShapeChanged()', 'Next identity rotation');
requireText(nextUi, 'receipt.sourceStartPosition', 'Next immutable receipt');
rejectText(nextUi, 'listForumTopicReplies', 'Next row-order inference');
rejectText(nextUi, 'parentReplyId', 'Next parent policy');

const leptosModel = read(
  'crates/rustok-forum/admin/src/topic_reply_range_model.rs'
);
requireText(
  leptosModel,
  'build_forum_reply_range_move_command',
  'Leptos model'
);
requireText(
  leptosModel,
  'Start position must not exceed end position',
  'Leptos range order'
);
requireText(
  leptosModel,
  'new_forum_reply_range_move_identity',
  'Leptos retry identity'
);

const leptosAdapter = read(
  'crates/rustok-forum/admin/src/transport/topic_reply_range_graphql_adapter.rs'
);
requireText(
  leptosAdapter,
  'moveForumTopicReplyRange',
  'Leptos GraphQL adapter'
);
rejectText(
  leptosAdapter,
  'forum_reply_range_move_operations',
  'Leptos audit boundary'
);
rejectText(leptosAdapter, 'native_server', 'Leptos native fallback');

const leptosUi = read(
  'crates/rustok-forum/admin/src/ui/topic_reply_range.rs'
);
requireText(leptosUi, '"FORUM-21X"', 'Leptos task marker');
requireText(leptosUi, 'type="number"', 'Leptos position inputs');
requireText(
  leptosUi,
  'rotate_command_identity',
  'Leptos identity rotation'
);
rejectText(leptosUi, 'fetch_topic_fork_replies', 'Leptos row-order inference');
rejectText(leptosUi, 'parent_reply_id', 'Leptos parent policy');

const transport = read('crates/rustok-forum/admin/src/transport.rs');
requireText(
  transport,
  'topic_reply_range_graphql_adapter::move_reply_range',
  'transport facade'
);
rejectText(
  transport,
  'topic_reply_range_native_server_adapter',
  'transport fallback'
);

const plan = read('crates/rustok-forum/docs/implementation-plan.md');
requireText(plan, 'Delivered through `FORUM-21X`', 'canonical plan');
requireText(
  plan,
  'FORUM-21X composes the bounded reply-range move command',
  'canonical plan delivery'
);
rejectText(
  plan,
  'public admin composition for the reply-range workflow',
  'canonical remaining scope'
);

console.log('FORUM-21X reply-range admin UI source contract is consistent.');
