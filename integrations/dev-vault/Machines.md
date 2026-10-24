# Machines

Cada máquina corre su propio servidor T3 (un *Environment*). T3 no comparte hilos ni proyectos entre Environments; este vault es la capa común.

Los agentes de un Environment **no** ven los hilos de otro. Para referirse a un hilo, anotar máquina + enlace `t3-thread://v1/<threadId>` en el `MEMORY.md` del proyecto.
