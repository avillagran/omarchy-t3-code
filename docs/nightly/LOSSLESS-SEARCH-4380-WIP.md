# Native UTF-16 search transport through nightly 4380

This isolated source candidate starts at client `b28b1d18e821b3fe2d8a57e20b48dee8ebde6f7e`.
It pins the exact public SDK `5867a17ef461cff94490362ecfbcea9d553ff3e5`,
validated by private Rust/C ABI/JNI/Kotlin gates and all seven public CI jobs
in draft SDK PR 42. Every canonical vendored crate leaf matches that commit;
there is no private SDK patch. Source preparation is not client acceptance.

## Data and ownership boundaries

Qt uses JSON syntax inside the existing scalar pipe envelope, as the native
search worker already does. The actual query, matches, snippets, native failure
causes and visit strings stay validated raw JSON in Rust. Legacy top-level
search/mount/visit packets are recognized before `serde_json::Value` decoding.
The public scalar search methods keep their original result shape and explicitly
refuse a result that requires raw JSON; the Qt path uses `resultJson` instead.
No isolated surrogate is replaced, dropped, or converted into a Rust `String`.

Query trim and 2..200 bounds use ECMAScript whitespace and UTF-16 units. The
actual UI retains its original `trim().slice(0, 200).trim()` expression. Existing
SDK option validators and per-function deadlines remain in use. Environment
order, independent failures, partial replies and terminal joins remain intact.
The trusted selected environment overwrites a server-supplied match environment.

Older-settled reads retain their complete original raw row beside scalar routing
metadata. Non-scalar text does not enter that metadata's String fields. Structural
metadata, identifiers and sequence gates remain distinct from text. The same
mount implementation enforces consumer holds, shared keys, replacement, release,
sequence cuts, removal versions, authoritative row precedence and atomic refusal.
Only the raw mount decoder can admit the internal raw channel; authoritative,
supplemental and ordinary Value mounts cannot spoof its marker. The normalizer
requires an actual String title in either its scalar or validated raw channel.
Qt restores original title/branch and the raw native search shell before
publishing the snapshot or constructing V8 search subjects.

Visits retain the existing ordered action queue and refresh path. Native results
and rejection causes are packed separately; a rejection remains an error. The
thread identifier, optional boolean and <=200-unit pick remain checked. Every
visitedAt String reaches the server unchanged, including date-only, zone-free,
invalid and empty values; its original DateTime.make owns validity. Other client
timestamp validators are unchanged.

## Actual Qt serialization boundary

The candidate does not assume the installed Qt engine implements well-formed
modern JSON.stringify. NativeJson quotes String values and property keys directly
from their UTF-16 code units. It retains ordinary BMP and valid pairs literally,
escapes isolated units and the original JSON special/control characters, and
uses the engine only for numeric/boolean formatting. This also keeps mount-key
syntax identical to modern JSON.stringify, rather than escaping every unit.
Object undefined fields are omitted and array holes/undefined become null.
The default toJSON hook is retained; packet callsites are plain metadata, arrays,
strings, flags and numbers. No object spread or Array.at is required.

The outer writer uses this path only for native-search, thread-search,
thread-search-mount and thread-visit. Other product send operations keep their
existing serializer. All nativeSearch callers were traced: query plans and opens
are plain records; palette metadata maps value/title/searchTerms/secondary and
JSON message matches; sidebar rankings carry keys, matches and shown order; row
shells originate in parsed JSON. Inner payload syntax is escaped before the outer
writer, so an isolated unit cannot become an unrepresentable Rust payloadJson
String or be repaired by QProcess UTF-8 conversion.

The prepared actual Qt gate records whether native JSON.stringify escapes,
retains or repairs an isolated high unit, plus low-unit and property-key behavior.
It separately requires the product codec to preserve nested high/low/reversed
units, pairs, NUL, property names, undefined/null-array behavior and numbers; it
then passes those encoded keys across the actual QProcess pipe and runs the
existing nativeSearch path with genuine V8 query/open results. These are prepared
assertions, not runtime observations yet.

## Original pick policy

The oracle is original nightly 4380 commit
`ade473018d32003658b7d38693e083ee2e314bd8`,
`packages/shared/src/paletteSearch.ts:770`. `pickedQuery` folds, slices to 32,
removes a trailing high surrogate, then trims. That intentional native policy
is separate from transport repair. The clipped unit-200 high surrogate must
survive query admission and RPC; a unit-240 snippet must survive the response.
Selection must send the actual V8 pick, including a low surrogate that the
original algorithm preserves. A pick is not forced to equal the full query.

## Prepared private gates; not executed

1. Strict all-target client metadata and a normal library build in the existing
   client target, only after the active owner's terminal and a fresh source-origin
   and artifact-reservation review. No main executable alias is released.
2. Link original client fixtures in container RAM against the actual new normal
   graph. Regress the existing three-peer search and mount fixtures. Run the new
   raw-row ownership fixture, including forged marker refusal.
3. Run `lossless-search-visit.rs` with the actual client search, mount,
   normalization, write and raw-visit paths; real SDK WebSocket peers; and the
   existing admitted Electron/V8 runtime with the original native pick function.
   Four queries cover unit200 clipping, lone low/high units and an intact pair.
   Actual server replies cover unit240 snippets, raw causes and forged scope.
   Every peer and the native worker must retire normally.
4. Run `lossless-search-transport.qml` in a private Quickshell/offscreen namespace
   with the same Rust fixture's `--qt-pipe` mode. It invokes the product
   ThreadSearch functions, real V8 query/open functions and the product visit
   packet helper. It requires exact Qt code units through query, result, mount,
   restored row, original pick and real visit, followed by normal child exit.

The small fixture routes the same library entry points as the application;
it does not yet constitute a complete application `watch` process or public
GUI acceptance. Main's raw routing and existing action queue also require the
actual all-target compiler graph; a later complete-watch gate must be qualified
separately if required for integration. No compiler or runtime has been launched
for this source candidate. No host tests, new Cargo target, executable copies,
cleanup, release, installation, active client edits or public selector changes
are included.
