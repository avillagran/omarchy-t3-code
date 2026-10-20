# T3 iconography

T3 custom provider marks (OpenAI, Claude, ACP, Cursor, Linux): copied as SVG
geometry from `apps/web/src/components/Icons.tsx` of pingdotgg/t3code,
commit `5a4e9980533705774dc3f45b35cacf76c3d78dbc`, release
`v0.0.46-nightly.20261014.3132`. License: T3-LICENSE (MIT).

Action/machine icons: Lucide geometry from the installed T3 nightly client
(0.0.46-nightly.20261013.3042), including server, cloud, monitor, laptop,
search, refresh, external link, close, git branch, sidebar collapse, paperclip,
arrow up, stop, sliders, file, image, send, list filter, folder plus and square pen.
Stroke 1.75 on the original 24px grid. See LUCIDE-LICENSE (ISC).

Assets remain local, immutable for this plugin version. Icons are tinted from
the active palette and rasterized/cached by Qt. No icon font substitutes,
extra process or per-frame SVG generation for bundled marks. ACP registry icons
follow the original picker: native metadata, only the official credential-free
CDN, Qt image cache and an alpha tint using the active Omarchy palette. Local
ACP entries keep the generic registry glyph. OpenCode paths match
packages/provider-opencode/src/client.ts in the local nightly source.

Project initials follow T3's projectIdentity.ts; project colors follow Omarchy's active accent, never the upstream fixed palette. Built-in
provider marks are trademarks of their respective owners; inclusion makes no
claim of affiliation.

T3 logo: exact SVG viewBox/path/fill from apps/web/src/components/T3Wordmark.tsx
at the same T3 commit above, MIT. Used in header and bar. It is separate from
Lucide PanelLeftClose/PanelLeft, which are interactive sidebar controls.

Sidebar PR/worktree glyphs use the official Lucide SVG geometry (folder-git-2,
layers, git-merge, git-pull-request-closed and git-pull-request-draft), retrieved
from lucide-icons/lucide on 2026-10-16, with the same 1.75 stroke as the other
T3-style action marks. The Lucide ISC notice applies.

Preferences navigation uses the Lucide keyboard SVG from lucide-icons/lucide
(retrieved 2026-10-16), with the same 1.75 stroke and ISC notice.

# Sounds

`sounds/notification-completion.mp3` and `sounds/notification-input.mp3` are copied
unchanged from `apps/web/src/assets/` of pingdotgg/t3code, commit
`fb990765a49414a5bfbdc73365455fc8b5fbe691`. License: T3-LICENSE (MIT).

# Mermaid

`mermaid/mermaid.min.js` is `dist/mermaid.min.js` from the npm package
`mermaid@11.17.2` (the version T3 Code pins; integrity
`sha512-V6K3C8EBdEsPFZXSKMJe6ppQOENxuHARr9GvHX4hh47lAbhMRD9qf4oEK7LoaRQxULMa80/qt5gHO73aCleBBg==`).
License: `mermaid/LICENSE` (MIT). It only runs inside the sandboxed document
renderer when you choose Show diagram on a Mermaid code block.
