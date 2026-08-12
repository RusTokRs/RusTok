from pathlib import Path

path = Path("scripts/tmp/apply-return-completion-order-owner-cutover.py")
text = path.read_text()
old = '''replace_once(
    core,
    "        journal: &ReturnCompletionOperationJournal,\\n        order_service: &OrderService,\\n        tenant_id: Uuid,\\n",
    "        journal: &ReturnCompletionOperationJournal,\\n        tenant_id: Uuid,\\n",
)
'''
new = '''replace_once(
    core,
    "    async fn execute_claimed(\\n        &self,\\n        journal: &ReturnCompletionOperationJournal,\\n        order_service: &OrderService,\\n        tenant_id: Uuid,\\n",
    "    async fn execute_claimed(\\n        &self,\\n        journal: &ReturnCompletionOperationJournal,\\n        tenant_id: Uuid,\\n",
)
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one ambiguous patch marker, found {count}")
path.write_text(text.replace(old, new, 1))
