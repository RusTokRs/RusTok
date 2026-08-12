# Forum Page Builder Wave live admission lineage actualization — 2026-08-12

## Base rechecked

`main@b446233033cf2981396e4502352111edb0aff43c`.

## Gap rechecked

The existing Forum Wave freshness verifier has a strict source-ready/live split and validates a live packet's declared admission format, status, source commit, deployment RepoDigest and packet SHA-256 field. Before this slice it did not open the retained `forum_page_builder_wave_admission_v1` packet whose SHA was declared by the live Wave packet, so exact admitted lineage at the live owner-review boundary still depended on a maintainer assertion rather than a fail-closed retained-file check.

## Source slice

Status: `forum-wave-live-admission-lineage-source-ready / maintainer-execution-pending`.

This slice adds `scripts/verify/verify-forum-wave-admission-lineage.mjs` and a machine-readable source contract. For `source_ready` Wave evidence the verifier preserves the current unexecuted admission cursor and does not require a fabricated packet. For `live` Wave evidence it requires `RUSTOK_FORUM_WAVE_ADMISSION_PATH` and fails closed unless:

- the live Wave `source_commit` equals checkout `HEAD`;
- the retained admission packet source commit equals both checkout `HEAD` and the live Wave;
- the live Wave `admission.packet_sha256` equals the SHA-256 of the actual retained admission file;
- the admission packet immutable RepoDigest equals the live Wave RepoDigest;
- all admission predecessor pass flags remain true;
- admission non-promotion boundaries remain false;
- admission privacy boundaries remain false;
- the live Wave latest-refresh gate set retains `node scripts/verify/verify-forum-wave-admission-lineage.mjs`.

The verifier does not upgrade RepoDigest equality into cryptographic origin/deployment provenance.

## Executable synthetic evidence

`verify-forum-wave-admission-lineage.test.mjs` invokes the production verifier and covers 9 cases:

1. source-ready evidence keeps the admission file pending;
2. valid live Wave + exact retained admission packet;
3. missing live admission packet path;
4. retained admission packet hash drift;
5. admission source-commit drift;
6. admission RepoDigest drift;
7. premature `forum_wave_accepted` claim in the admission packet;
8. admission privacy overclaim;
9. missing retained admission-lineage verifier gate in the live refresh record.

These fixtures test verification behavior only. They are not live Pages/Forum evidence and do not execute a control-plane Wave.

## CI discipline

The new source-ready verifier and synthetic suite are appended to the existing read-only `Pages Page Builder Provider Health` workflow. No new workflow is created; its existing `concurrency.cancel-in-progress: true` policy remains the superseded-run cleanup mechanism available through repository source.

## Non-claims / next cursor

This slice does **not** execute the Pages reference candidate/gate, Forum browser evidence, runtime-authorization Cargo commands, server-function deployment attestation, Forum admission against a deployment, observed control-plane Wave, approvals, rollback action or owner review. It does not assert current provider health, cryptographic origin-to-RepoDigest binding, Forum Wave acceptance or FFA/FBA promotion.

Next execution cursor remains:

1. execute/accept live Pages gate inputs and Forum browser/runtime/server-function packets on one exact source/deployment boundary;
2. retain the real Forum admission packet;
3. bind the observed live Wave packet to that exact retained admission file with this verifier plus the existing freshness verifier;
4. retain control-plane audit/fallback/metrics/traces/rollback/approvals/waivers and take owner review;
5. promote only after accepted observed evidence.
