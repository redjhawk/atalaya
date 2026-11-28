# UC-TELEMETRY-002 Enviar un fix al panel

Especificación: `docs/specifications/telemetry-intake/functional.md`.

## Actor

Cualquier cliente que llega al listener del panel, con o sin la cabecera de huella.

## Precondiciones

- El server está en marcha.
- Puede existir un device dado de alta con una huella (UC-ENROL-001).

## Flujo principal

1. El cliente envía al panel `POST /telemetry/{device}` con un fix válido y, en la cabecera de huella,
   la huella registrada para ese device.
2. El panel no tiene esa ruta y no lee la cabecera de huella: responde `404 Not Found`.

## Flujos alternativos

- **A1 Sin cabecera**: la misma petición sin cabecera de huella también recibe `404 Not Found`.

## Flujos de error

- Ninguno distinto del flujo principal: toda petición a la ruta del device en el panel termina en
  `404`.

## Postcondiciones

- No se guarda ningún fix y no cambia el último contacto de ningún device.

## Requisitos cubiertos

FR-TELEMETRY-006, FR-TELEMETRY-008, FR-ENROL-016.
