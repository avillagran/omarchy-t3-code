# Nightly 4380 palette navigation and remaining catalog work

The Qt consumer now carries the original submenu search state, parent query
restoration, scoped search with all-options fallback, nested suggestions,
disabled row keyboard exclusion and `keepOpen` execution. The native worker
calls the unchanged original `buildNestedSearchItems` and
`filterCommandPaletteGroups`; it decorates nested thread subjects recursively.
The empty root view shows commands and recent threads; project/settings search
inventories remain available to the native search matcher and submenu fallback.

Four complete pure declarations from original `CommandPalette.logic.ts` at
`ade473018d32003658b7d38693e083ee2e314bd8` are generated for Qt through TypeScript
erasure, export removal, standard Babel object-spread lowering and an Array.at
coercion/bounds polyfill. The generator checks the whole original source hash,
exact declaration membership and closure bindings. The original MIT license is
retained. Source generation passed in a private container; this alone does not
establish Qt consumer acceptance.

The palette sends query changes directly to the shared 120 ms backend debounce,
removing an additional 250 ms UI delay. The input accepts pasted queries beyond
the former 256-character cap; server queries still use the original 200 UTF16
unit normalization. IME composition does not execute a highlighted command.

Strict all-target checking, the normal library and canonical main compilation
passed. The first actual Qt gate exposed unsupported object-spread syntax before
loading the palette; standard compiler lowering now addresses that parser gap.
The next gate reached scoped fallback and exposed the missing Array.at builtin
when returning from a submenu. A standard compatibility helper now preserves
that original operation; the corrected Qt fixture passed with the actual
canonical main, admitted native V8 worker and private Wayland compositor. The
extended fixture exercises original nested matching, scoped fallback, Backspace
restoration, disabled actions, retained-open actions and a 1,000-character paste.
It supplies native-shaped menu entries; it is not acceptance of the complete
Nightly action catalog or project/theme/browse controllers. Those integrations,
shared query subscriptions, raw UTF16 app transport and full global parity remain
separate pending work.

Accepted receipts:

- Compiler: `/var/tmp/o3-build/primary-palette-navigation-4380-compile2-pbtIUlsQ/output/result.json`, SHA256 `c01608886b6853e1e69e27518b560e57df2889ac62c687689d3b1fd33c1d331d`; strict all-targets, normal library and canonical main example passed in 35.53 seconds.
- Actual main SHA256 `f188f9de45975817cab6bc5b3d2970c9e673d988c99680bf8b78b2f9abcfa95c`; only the two terminal previous main aliases were replaced, with 27 other aliases read-only. The new main is retained as part of the 29 protected aliases.
- Qt: `/var/tmp/o3-build/primary-palette-navigation-4380-runtime3-2A4wiYPy/output/result.json`, SHA256 `9af4079f0a7bcb7cb8481d5ce928eb2dd469018a0fd97720104deeab8b3a6106`; the full extended palette fixture passed in 15.06 seconds. All 406 actual graph files and 29 executable aliases remained unchanged with the entire target read-only.

The failed parser/Array.at gates and the generator closure-check failure remain
retained. No assertions were removed. Rust, Cargo, vendor and worker bytes stayed
identical to the accepted compiler while only Qt compatibility source changed;
unchanged mount and worker-protocol tests were not repeated. No runtime copies,
new Cargo target, deployment or cleanup were performed.
