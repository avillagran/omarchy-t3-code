# Restored controls and preference persistence

The integration branch now contains the three controls missing from the earlier
Git audit:

- `85a1a24`: OmaBridge sharing and unavailable text identify T3/O3, with the
  existing twenty-language catalogs updated together.
- `26ce3f4`: preferences reopen on their last selected page.
- `148e407`: provider device-code sign-in exposes the existing Copy code action,
  preserving the current browser and pasted-code flow.
- `b929bd5`: the Rust presentation-state sanitizer retains the twelve supported
  preference pages. The existing worker-restart integration assertion also
  checks the selected page; that broader test has not run against a rebuilt app.

## Scoped validation

Two existing QML cases passed with current sources in a private compositor:
`provider-setup` and `panel-reopen`, including ten reopen cycles. They used the
retained CLI80 executable and synthetic provider metadata. No live account,
host display or newly linked application was involved.

Receipt: `/var/tmp/o3-build/restored-ui-4367-pvjbuxlv/result.json`, SHA256
`60189a0d8f41222eb7379bc9e17f673b6086c8615bed271f99cb25049bd3a6ed`.
All 1,740 pinned inputs remained unchanged; the actual container terminated.

The complete current `src/ui_state.rs` module also passed actual filesystem
save/load in four separate private processes: save/read a supported page and
save/read an unsupported page. The valid page and language survived; invalid
page, transcript and credential fields were excluded. This used the actual
module with the normal client library graph, through a RAM-only probe.

Receipt: `/var/tmp/o3-build/preference-state-4367-3-aez8r73b/result.json`, SHA256
`3a6903b41bf4dfa9bd474ff9a0002c8a0f9e7517e0dcfb8971afa756e0b7b5b3`.
All 2,312 inputs remained unchanged. Earlier pre-runtime failures are retained:
the first graph was stale; the second probe omitted the actual libc extern.
Neither executed the persistence cases. The successful module probe does not
replace the pending rebuilt-application worker integration test.

## Shared Cargo cache finding

The first normal library build exposed a stale local SDK artifact. Cargo marked
it fresh despite three current public APIs being absent from its actual symbols.
Relative dep-info and source mtimes allowed artifacts from another worktree to
survive a source switch in the shared target.

Under the existing exclusive build lock, exactly one regular sixteen-byte
`lib-t3_protocol` fingerprint was renamed, on the same filesystem, to
`/home/node/BuildCache/o3-client-voice-4314/target/o3-primary-library-4367-2-fingerprint-backup`.
Its identity and bytes were retained. No tree, source, executable or library was
deleted, and no new target or manually compiled SDK was introduced.

Normal `cargo build --locked --offline --lib --jobs 2` then passed. Actual SDK
and root artifact records both report `fresh: false`; the admitted graph records
384 library, metadata and native files. All nineteen reserved executable/plugin
paths and source fences remained intact. This build ran no tests or GUI.

Receipt: `/var/tmp/o3-build/primary-library-4367-2-output/result.json`, SHA256
`8a5649505aa60abf3fbe9a4266ea7d73529983193a4f9721f1b41dd10072f209`.
Actual graph SHA256:
`7b97bf423d487a79f6e5f12b7481255251c49c4166de65274a5634a957e8eacc`.

Future builds that switch local path sources in this shared target must check
source provenance before trusting freshness. Combined GUI acceptance, rebuilt
application validation and publication of these client commits remain pending.
