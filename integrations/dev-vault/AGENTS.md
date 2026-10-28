# AGENTS.md — Protocolo del Dev Vault

Este vault es un repositorio git compartido entre todas las máquinas de desarrollo (ver [[Machines]]). Es la **fuente de verdad** de estado, decisiones y avances de cada proyecto. T3 Code guarda los hilos por máquina; lo que no esté aquí se pierde al cambiar de equipo.

## Al iniciar una sesión de trabajo en un proyecto

1. `git -C ~/Obsidian/Dev pull --rebase --quiet` (si falla por red, seguir y avisar).
2. Leer [[Memory/User]] y [[Memory/Rules]].
3. Leer `Projects/<slug>/MEMORY.md`. Si no existe, crearlo desde [[Templates/Project-MEMORY]].
4. Revisar en ese MEMORY la sección **Hilos relevantes**: si el hilo útil está en otra máquina, decirlo al usuario en vez de rehacer el trabajo.

`<slug>` = título del proyecto en T3 (carpeta del repo). Un proyecto que existe en varias máquinas usa **una sola** carpeta.

## Consultar el vault sin cargarlo entero

- Buscar antes de leer: `rg -n -i '<término>' ~/Obsidian/Dev --glob '*.md'` y abrir solo las notas encontradas.
- Hilos anteriores de un proyecto, en todas las máquinas: `Projects/<slug>/Threads.md` (lo mantiene O3 Code: los 25 más recientes con máquina y enlace `t3-thread://`; no editarlo). Más antiguos: `git log -p -- Projects/<slug>/Threads.md`.
- Decisiones y estado: `Projects/<slug>/MEMORY.md`; detalle de una sesión: `Projects/<slug>/Sessions/`.

## Al terminar, o tras un avance significativo

1. Actualizar `Projects/<slug>/MEMORY.md`: **Estado**, **Decisiones** (con fecha y por qué), **Pendientes**, **Hilos relevantes** (máquina + `t3-thread://v1/<threadId>` + título).
2. Si hubo trabajo sustancial, crear `Projects/<slug>/Sessions/YYYY-MM-DD-<topic>.md` desde [[Templates/Session]] con qué se hizo, qué se verificó y qué queda.
3. Commit y push:
   ```sh
   cd ~/Obsidian/Dev && git add -A && git commit -qm "journal(<slug>): <resumen corto>" && git pull --rebase -q && git push -q
   ```
   Si el push falla, dejar el commit local y reportarlo. Nunca forzar.

## Reglas

- **Nombres de archivos y carpetas en inglés.** Contenido en español.
- Sin secretos (tokens, contraseñas, API keys, URLs de pairing) y sin volcados de datos; sólo markdown y diagramas pequeños.
- Notas cortas (< 300 líneas). Decisiones como viñetas fechadas, no prosa larga.
- No reescribir historia: corregir añadiendo una entrada nueva con fecha.
- Un agente no edita `Memory/User` ni `Memory/Rules` salvo pedido explícito.
- Enlazar con `[[wikilinks]]`; usar `#status/active`, `#status/paused`, `#status/done`.
