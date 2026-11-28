# UC-ENROL-004 Arrancar el server

Especificación: `docs/specifications/device-enrolment/functional.md`.

## Actor

Operador, en la máquina del server.

## Precondiciones

- Ninguna; los devices pueden estar dados de alta o no.

## Flujo principal

1. El operador ejecuta `atalaya-server` sin argumentos.
2. El server arranca como hasta ahora: crea y migra la base de datos si no existe y abre el device API
   (`ATALAYA_DEVICE_PORT`, por defecto 8080) y el panel (`ATALAYA_PANEL_PORT`, por defecto 8081).
3. El device API sirve solo `GET /healthz` y `POST /telemetry/{device}`; el panel sirve solo
   `GET /healthz`.

## Flujos alternativos

- **A1 Alta por red**: un cliente cualquiera intenta crear un device o cambiar o revocar su huella con
  una petición HTTP a cualquier listener. No existe ninguna ruta para ello: la petición no crea ni
  modifica ningún device.

## Flujos de error

- **E1 Subcomando desconocido**: el operador ejecuta `atalaya-server` con un argumento que no es un
  subcomando conocido (p. ej. `atalaya-server devices`). Se escribe en la salida de error el uso de los
  tres subcomandos, termina con código distinto de 0 y el server no arranca.

## Postcondiciones

- Éxito: los dos listeners escuchan; los devices solo se crean o modifican por los subcomandos.
- Error: ningún listener abierto.

## Requisitos cubiertos

FR-ENROL-001, FR-ENROL-014, FR-ENROL-016.
