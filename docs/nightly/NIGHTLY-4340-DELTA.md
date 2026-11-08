# Nightly 4340 source delta

The target is `v0.1.8-nightly.20261107.4340` at
`a9e709f15f825cacac1b6d665fa1920c328d68b7`. The original 4314 checkout
and its accepted receipts are retained. The exact source comparison finds 70
unchanged original footprints, 152 changed paths and 28 commits. The complete
evidence and per-commit paths are in `reference-delta-4340.json`.

An unchanged original footprint retains only its earlier qualified component
evidence. It does not accept changed schemas, dependencies, callers, or the
whole application. The global validated pin and SDK vendor remain unchanged.
The table records new work explicitly; pending review makes no assertion about
whether an existing O3 implementation already handles that change. Mobile work
requires an explicit platform/Android review and is not silently excluded.

| Commit | Original change | Current evidence |
| --- | --- | --- |
| `a9e709f1` | fix(mobile): center Android header confirmation buttons (#32239) | pending review |
| `af96a01a` | fix(mobile): shorten the model picker save action (#32551) | pending review |
| `0080084c` | fix(mobile): group settled PR threads in the continue menu (#27941) | pending review |
| `511238c3` | fix(mobile): Android terminal sends hardware-keyboard arrows, Tab, and Escape to the PTY (#32666) | pending review |
| `25bf84ab` | fix(mobile): center the Choose project empty-state buttons (#27450) | pending review |
| `e4552000` | fix: side chats start with the parent's permission mode and reasoning level (#32315) | pending review |
| `97d04c3b` | fix(forgejo): tea's `pull` and `m` aliases count as shell merges (#32401) | pending review |
| `a601caaf` | fix(gitlab): `glab mr accept` counts as a shell merge (#32400) | pending review |
| `917ce994` | fix(web): agent sign-in defaults to Auto access (#32186) | pending review |
| `4c3a9d58` | fix(cursor): use the SDK account for usage data (#32448) | pending review |
| `9f00ced8` | fix(antigravity): explain missing SOCKS dependency during refresh (#19660) | pending review |
| `f016a16b` | fix(acp): preserve native form questions and typed answers (#19659) | pending review |
| `e3441535` | fix(antigravity): check account access after installation (#32446) | pending review |
| `67fdc538` | fix(desktop): published themes reach the app when it uses this machine's service (#26324) | pending review |
| `eeb6e236` | fix(web): keep Pi thinking controls visible before selection (#27397) | pending review |
| `beaeeff4` | fix(acp): bound unread raw notification retention (#19585) | pending review |
| `5cb2a7d0` | fix(pi): preserve explicit thinking when switching models (#32389) | pending review |
| `2d0a9dfd` | fix(provider-acp): preserve UTF-8 across interleaved terminal streams (#32391) | pending review |
| `c864b1c1` | feat(web): delete archived threads from the archive page (#22160) | accepted component awaiting publication |
| `95e7dbfd` | fix(github): keep review-required until required reviewers approve (#32395) | pending review |
| `a9752171` | fix(grok): long command titles no longer end in a broken character (#32408) | pending review |
| `ce9b8067` | fix(server): theme files saved with a UTF-8 BOM are published instead of rejected (#32405) | pending review |
| `48bb784e` | fix(shared): inline code like `allocator.cc:217` becomes a file chip (#32403) | pending review |
| `7f7bf263` | fix(web): recover ACP settings when projects change (#32409) | pending review |
| `fd1adc88` | fix(acp): retain streamed child tool output (#32411) | pending review |
| `1b952135` | fix(providers): remove incomplete ChatGPT connections (#29280) | source implementation |
| `bc46c0ff` | perf(server): avoid lsof polling when Linux proc is usable (#30011) | pending review |
| `fad24fc8` | fix(azure-devops): default-branch lookup no longer fails for missing --repository (#29586) | pending review |

The archive slice uses actual Qt controls and inert transport receipts; it
deletes no real thread. The new managed auth removal consumer must use actual
methods and `canRemove` data. Existing local CLI controllers do not own the
managed registration store and must refuse unsupported removal truthfully.
Full physical browser STATUS, GUI input context, transport bounds, cloud and
emulator work remain tracked independently from this incremental source delta.
