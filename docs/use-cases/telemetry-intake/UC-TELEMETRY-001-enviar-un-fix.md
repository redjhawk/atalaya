# UC-TELEMETRY-001 Enviar un fix

Especificación: `docs/specifications/telemetry-intake/functional.md`.

## Actor

Device, a través del locator. El edge valida el certificado de cliente y reenvía su huella en la
cabecera `ATALAYA_FINGERPRINT_HEADER` (por defecto `x-client-cert-sha256`).

## Precondiciones

- El device está dado de alta con una huella (UC-ENROL-001).
- El server está en marcha.

## Flujo principal

1. El device envía al device API `POST /telemetry/{device}` con su id en la ruta, su huella en la
   cabecera y un fix en `location`, p. ej.
   `{ "location": { "lat": 40.0, "lon": -3.0, "acc": 12.5, "ts": 1700000000000, "prov": "fused" } }`.
2. El server comprueba que la huella de la cabecera es la registrada en ese momento para el
   `{device}` de la ruta.
3. El server valida el fix: `lat` en [-90, 90], `lon` en [-180, 180], `acc` ≥ 0, `ts` entero positivo y
   `prov` uno de `gps`, `fused`, `network`.
4. El server guarda el fix para ese device con sus cinco campos y el momento de la recepción.
5. El server fija el último contacto del device en el momento de la recepción.
6. El server responde `201 Created`.

## Flujos alternativos

- **A1 Campos extra**: el cuerpo trae otras secciones de telemetría o `location` trae otros campos. Se
  ignoran y el flujo sigue.
- **A2 Huella cambiada con el server en marcha**: el operador sustituyó la huella (UC-ENROL-002). La
  comprobación del paso 2 usa la huella nueva desde la siguiente petición.

## Flujos de error

En todos, no se guarda ningún fix y no cambia el último contacto de ningún device.

- **E1 Identidad rechazada** (paso 2): la petición no trae cabecera de huella, trae una huella que no es
  de ningún device, trae la huella de otro device, va a un `{device}` que no existe o a un device
  revocado. El server responde `401 Unauthorized`, con el mismo estado y cuerpo en los cinco casos. Se
  comprueba antes que el cuerpo: la respuesta es `401` aunque el cuerpo también sea inválido.
- **E2 Fix inválido** (paso 3): con la huella correcta, el cuerpo no es JSON, no trae `location`, falta
  un campo o un campo incumple las reglas (p. ej. `lat` 91, `lon` -181, `acc` -1, `ts` 0, `prov`
  desconocido). El server responde `400 Bad Request`.

## Postcondiciones

- Éxito: el fix está guardado para el device y su último contacto es el momento de la recepción.
- Error: ningún fix nuevo y ningún último contacto cambiado.

## Requisitos cubiertos

FR-TELEMETRY-001, FR-TELEMETRY-002, FR-TELEMETRY-003, FR-TELEMETRY-004, FR-TELEMETRY-005,
FR-TELEMETRY-006, FR-TELEMETRY-007, FR-ENROL-005, FR-ENROL-006.
