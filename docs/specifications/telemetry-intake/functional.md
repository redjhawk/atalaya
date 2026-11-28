# Recepción de fixes autenticada por huella

Origen: issue #12. Decisión de identidad: `docs/adr/0003-device-mtls-behind-cloudflare.md`.

## Propósito

El device envía un fix al server y el server lo guarda solo si la petición viene del device que dice
ser: la **huella** que el edge reenvía en la cabecera debe ser la registrada para el `{device}` de la
ruta. Cualquier otra petición se rechaza sin guardar nada.

## Actores

- **Device** (a través del locator): envía la telemetría por el device API.
- **Edge** (Cloudflare): valida el certificado de cliente y reenvía su huella en la cabecera
  configurada en `ATALAYA_FINGERPRINT_HEADER` (por defecto `x-client-cert-sha256`).

## Alcance

- `POST /telemetry/{device}` en el device API, con un fix en el campo `location`.
- Actualización del último contacto del device.

## Exclusiones

- Otras secciones de telemetría (batería, red, escaneos, uso de apps, integridad, pantalla): se
  ignoran, no se guardan ni se validan.
- Envío de varios fixes en una petición, deduplicación por ID de cliente (cola offline).
- Consulta de fixes, mapa, comandos, WebSocket y SOS.

## Petición

Cuerpo JSON. El fix va en `location`; los demás campos del cuerpo y de `location` se ignoran.

```json
{ "location": { "lat": 40.0, "lon": -3.0, "acc": 12.5, "ts": 1700000000000, "prov": "fused" } }
```

Reglas de un fix válido (todos los campos son obligatorios):

- `lat`: número entre -90 y 90, ambos incluidos.
- `lon`: número entre -180 y 180, ambos incluidos.
- `acc`: número mayor o igual que 0, en metros.
- `ts`: entero positivo, milisegundos desde la época Unix, momento de la lectura en el device.
- `prov`: uno de `gps`, `fused`, `network`.

## Comportamiento

- **FR-TELEMETRY-001**: Una petición cuya cabecera de huella es exactamente la huella registrada para
  el `{device}` de la ruta y cuyo cuerpo trae un fix válido se acepta con `201 Created`, y el fix queda
  guardado para ese device con sus cinco campos y el momento en que el server lo recibió.
- **FR-TELEMETRY-002**: Una petición aceptada fija el último contacto del device en el momento en que
  el server la recibió.
- **FR-TELEMETRY-003**: Se rechaza con `401 Unauthorized` la petición:
  - sin cabecera de huella,
  - con una huella que no es la de ningún device,
  - con la huella de otro device,
  - a un `{device}` que no existe,
  - a un device revocado.

  Las cinco respuestas son idénticas (mismo estado y mismo cuerpo), para no revelar qué devices ni qué
  huellas existen.
- **FR-TELEMETRY-004**: La identidad se comprueba antes que el cuerpo: una petición que no supera la
  comprobación de huella recibe `401` aunque su cuerpo sea inválido.
- **FR-TELEMETRY-005**: Una petición con la huella correcta cuyo cuerpo no es JSON, no trae `location`
  o trae un fix que incumple las reglas se rechaza con `400 Bad Request`.
- **FR-TELEMETRY-006**: Una petición rechazada, por cualquier motivo, no guarda ningún fix y no cambia
  el último contacto de ningún device.
- **FR-TELEMETRY-007**: La comprobación usa la huella registrada en el momento de la petición: una
  sustitución o revocación hecha con el server en marcha se aplica a la siguiente petición.
- **FR-TELEMETRY-008**: La cabecera de huella solo se tiene en cuenta en el device API. En el panel no
  existe `POST /telemetry/{device}`: la petición recibe `404 Not Found`, con o sin cabecera de huella,
  y no guarda nada.

## Criterios de aceptación

1. Un fix válido con la huella registrada del device recibe `201`, queda guardado con sus cinco campos
   y el último contacto del device pasa a ser el momento de la recepción.
2. Sin cabecera, con huella desconocida, con la huella de otro device, a un device inexistente o a un
   device revocado: `401` idéntico, ningún fix guardado y último contacto sin cambios.
3. Con la huella correcta y `lat` 91, `lon` -181, `acc` -1, `ts` 0, `prov` desconocido, `location`
   ausente o cuerpo no JSON: `400`, nada guardado y último contacto sin cambios.
4. La misma petición válida enviada al panel, con la cabecera de huella registrada, recibe `404` y no
   guarda nada.
5. Los tests usan huellas de ejemplo evidentes y coordenadas ficticias; el repo no contiene huellas ni
   ubicaciones reales.
