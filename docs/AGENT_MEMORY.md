# Agent memory in OT3

Preferences → General → Agent memory controls OT3’s own runtime policy. It is
enabled by default after five minutes; choose another interval or Never, globally
or for a provider instance on the selected environment. The policy runs while
OT3 is running, including when its window is hidden. It uses existing native
session operations and does not wait for an upstream T3 release.

Release retains the provider’s native conversation ID. The next message resumes
the saved conversation. It does not delete history, discard drafts, settle the
thread, change the selected conversation, or send a compaction message.
Active and queued runs, questions, approvals, proposed plans and reported
background work postpone release. Unknown ACP agents are skipped unless the
adapter exposes durable loading. Decisions are rechecked against fresh bounded
state; claims prevent replay after a lost reply or worker restart.

The existing detach operation on older servers has no atomic idle precondition.
A turn started from another client between the final read and detach can race
with release. The server-side idle timers avoid that cross-client admission gap;
OT3’s checks cannot turn a legacy RPC into an atomic transaction.

## Compaction follows the harness

Codex: OT3 does not compact just because the thread is idle. Codex’s native
context-window compaction remains enabled. Indefinite cache retention announced
for ChatGPT-authenticated Codex is distinct from API cache policy and from the
model’s finite context window.

Claude: preventive compaction uses the reported cache lifetime first and the
configured per-provider estimate otherwise. Claude subscriptions generally use
one hour for the main conversation; API/usage-credit requests and most auxiliary
requests use five minutes. A saved conversation can resume after its cache
expires; the next turn may process more input tokens. Closing the process and
compacting the context are independent actions.

Other harnesses: OT3 checks the provider’s advertised compact command. A generic
ACP implementation is not assumed to support compaction or durable recovery.

Diagnostics: `o3 call memoryState` reports the local policy and worker status.
