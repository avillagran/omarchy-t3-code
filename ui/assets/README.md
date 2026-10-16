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
