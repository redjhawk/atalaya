# Issue #12 Server: alta de device por CLI y primer fix autenticado por huella

## Parent

#2

## What to build

Un operador da de alta un device con su huella de certificado desde la línea de comandos, y ese device puede enviar un fix que queda guardado. Cualquier otra petición se rechaza. Es el primer corte completo: alta, identidad, recepción y persistencia.

Toca la identificación del device: el PR requiere aprobación humana (`AGENTS.md`).

## Acceptance criteria

- [ ] Un subcomando del binario registra un device con id, nombre y huella.
- [ ] Un subcomando sustituye o revoca la huella de un device existente.
- [ ] `POST /telemetry/{device}` con una ubicación válida y la huella registrada para ese device guarda el fix.
- [ ] Se rechaza, sin guardar nada, la petición sin cabecera de huella, con huella desconocida y con la huella de otro device.
- [ ] Las rutas del device no existen en el listener del panel: enviar allí la cabecera de huella no da acceso.
- [ ] Una petición aceptada actualiza el último contacto del device.
- [ ] No existe ningún endpoint de alta por red.
- [ ] Los tests usan huellas de ejemplo; no hay certificados, huellas reales ni ubicaciones reales en el repo.

## Blocked by

- #11
- #9


## Comments by trusted authors

