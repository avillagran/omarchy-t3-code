# Native palette search compatibility at Nightly 4367

The complete upstream search module passes its original 34 synchronous test
callbacks and 192 assertions in pinned V8. Direct QML execution is incompatible
with the module's regular expressions. No application consumer has changed;
the search feature remains pending integration.

## Preserved source

Local checkpoint `558961449cb2444074980c6a081fae2f97ec8768`, at
`refs/checkpoints/20261108/native-palette-search-v8-investigation`, preserves six
source files without changing the selected HEAD or real index. It includes the
build generator, the complete 19-export search module, its original test
callbacks, two source receipts and a QML fixture. The generated module is a
compatibility investigation, not an admitted QML implementation.

The pinned upstream revision is `6417151c093e54dbba0e6c824c69d03983793b9a`.
The generator verifies the complete `paletteSearch.ts`, its original test file,
the native branch-prefix dependency, license and existing compiler lock. It
erases TypeScript and links the complete module; it does not replace the search
algorithm with a local scoring approximation. The compatibility transforms and
every source hash are recorded in the generated source receipts.

## Actual observations

| Observation | Result and scope |
| --- | --- |
| Private source generation | Pass; original 34 test callbacks retained, 6,113 admitted inputs unchanged. |
| Original callbacks in Node 26.13.0 V8 | Pass; 34 callbacks, 192 assertions, no skipped expectations. This uses explicit assertion adapters, not the upstream test runner. |
| Original first callback in QML | Fail; title order differs from the upstream expectation. |
| Separate Qt diagnostic | Unicode property expressions fail to recognize ordinary letters; lookbehind construction throws `SyntaxError`. Ordinary floating-point sorting and `Math.log2` work. This is diagnostic evidence only. |

The Qt failure changes word-boundary classification, so porting only the score
ordering cannot establish parity. Narrowing the input to ASCII or dropping
original test cases would not preserve the native behavior. A compatible local
V8 execution boundary and its Qt consumer still need implementation and
validation.

Earlier failed attempts are retained. The first private QML launch failed
before running a case because its private runtime directory was missing. The
next attempt was rejected before launch by an invalid tmpfs option. After
correcting those environment issues, the first original assertion failed; its
immediate startup exit did not terminate Quickshell before the namespace bound.
The separate diagnostic uses a post-startup timer and terminates normally.
An initial V8 assertion adapter also mishandled the optional assertion message;
the corrected adapter passes all original callbacks with identical search and
test-module bytes.

## Remaining integration

The existing command palette and sidebar still use their earlier scoring
implementations. Full parity also requires the native thread-subject metadata,
pull-request hints, opened/search-query visit history, local open deduplication,
late-result ordering and native group limits. The current SDK visit command and
server visit projection do not yet carry the complete upstream open history.
Core conformance alone does not close these consumers or persistence paths.

## Evidence

All paths below are under `/var/tmp/o3-build/`. Each private runtime completed
with the same owned Podman client, name and actual container ID terminal and
admitted inputs unchanged. No live account or host display was used.

- `palette-search-native-build-4367-3/result.json`: SHA256 `5508d1c68ab935bbcde774be23aa349bd9fabe8a9c296d2f0f0618154581f418`.
- `palette-search-native-node-4367-2/result.json`: SHA256 `a8d0cc6bcff06b19d2a12974792370df4b4989c0af934c0e65bf45833c2de574`.
- `palette-search-native-qt-diagnostic-4367/result.json`: SHA256 `2fe1503eebc6990512117d4743d0bbce7be649697f38d983efd3cc2a8308d052`.
- `native-palette-checkpoint-v2-594zl6du/receipt.json`: SHA256 `ed104cd06c66ba5121807db5ff088125b03f8236cd332c795f4ace38048ee715`.
