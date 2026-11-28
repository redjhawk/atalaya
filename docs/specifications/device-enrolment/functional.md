# Alta de device por línea de comandos

Origen: issue #12. Decisión de identidad: `docs/adr/0003-device-mtls-behind-cloudflare.md`.

## Propósito

El **operador** da de alta el device en el server, y sustituye o revoca su **huella**, desde la línea
de comandos del propio binario del server. El alta es manual y presencial (`SCOPE.md`): no existe
ningún camino por red para crear o modificar un device.

## Actores

- **Operador**: la persona que administra el server y despliega el device. Ejecuta los subcomandos
  en la máquina del server.

## Alcance

- Alta de un device con id, nombre y huella.
- Sustitución de la huella de un device existente.
- Revocación de la huella de un device existente.

## Exclusiones

- Listar, renombrar o borrar devices.
- Alta, sustitución o revocación por red (ningún endpoint, en ningún listener).
- Seguimiento de la caducidad del certificado.
- Generación o firma del certificado (pasos de `adb` y de Cloudflare del ADR 0003).

## Reglas de entrada

- **id**: de 1 a 64 caracteres; solo minúsculas `a-z`, dígitos `0-9` y guion `-`; empieza por letra o
  dígito. Es el `{device}` de las rutas del device API.
- **nombre**: texto libre de 1 a 100 caracteres, que no puede estar vacío ni ser solo espacios.
- **huella**: exactamente 64 caracteres hexadecimales (SHA-256). Se aceptan mayúsculas y minúsculas y
  se guarda en minúsculas, la forma en que el edge la reenvía. Cualquier otro carácter (`:`, espacios)
  la hace inválida.

## Comportamiento

- **FR-ENROL-001**: Ejecutar el binario sin argumentos arranca el server como hasta ahora.
- **FR-ENROL-002**: `atalaya-server device add <id> <nombre> <huella>` crea un device con ese id,
  nombre y huella, sin fixes y sin último contacto. Termina con código 0 y escribe en la salida
  estándar una línea que confirma el alta e incluye el id.
- **FR-ENROL-003**: `atalaya-server device set-fingerprint <id> <huella>` sustituye la huella del
  device por la nueva, tenga o no una huella antes. Termina con código 0 y confirma en una línea con el
  id. El nombre, los fixes y el último contacto del device no cambian.
- **FR-ENROL-004**: `atalaya-server device revoke <id>` deja el device sin huella. Termina con código 0
  y confirma en una línea con el id. Revocar un device ya revocado también termina con código 0. El
  nombre, los fixes y el último contacto del device no cambian.
- **FR-ENROL-005**: Tras una sustitución, las peticiones con la huella anterior se rechazan y las
  peticiones con la nueva se aceptan (ver `docs/specifications/telemetry-intake/functional.md`).
- **FR-ENROL-006**: Tras una revocación, toda petición de ese device se rechaza, sea cual sea la
  huella, hasta que se le asigne una huella con `set-fingerprint`.
- **FR-ENROL-007**: El efecto de un subcomando se aplica a la siguiente petición que reciba un server
  en marcha sobre la misma base de datos, sin reiniciarlo.
- **FR-ENROL-008**: Los subcomandos usan la base de datos de `ATALAYA_DATABASE_PATH` (por defecto la
  misma que el server) y la crean y migran cuando no existe, igual que el arranque del server.
- **FR-ENROL-009**: Los subcomandos no abren ningún listener.

## Flujos de error

Todo error termina con código distinto de 0, escribe un mensaje en la salida de error que nombra la
causa, y no modifica la base de datos.

- **FR-ENROL-010**: `device add` con un id que ya existe.
- **FR-ENROL-011**: `device add` o `device set-fingerprint` con una huella que ya tiene otro device. Una
  huella identifica a un solo device. Una huella revocada queda libre y puede asignarse a otro device.
- **FR-ENROL-012**: `device set-fingerprint` o `device revoke` con un id que no existe.
- **FR-ENROL-013**: id, nombre o huella que incumplen las reglas de entrada.
- **FR-ENROL-014**: Un subcomando desconocido, o un número de argumentos distinto del esperado,
  escribe en la salida de error el uso de los tres subcomandos y no arranca el server.
- **FR-ENROL-015**: `device set-fingerprint` con la huella que el device ya tiene termina con código 0
  y no cambia nada.

## Sin alta por red

- **FR-ENROL-016**: Ninguna ruta de ningún listener crea un device ni cambia o revoca su huella. El
  device API solo sirve `GET /healthz` y `POST /telemetry/{device}`; el panel solo sirve `GET /healthz`.

## Criterios de aceptación

1. `device add` con valores válidos termina con código 0, y un fix enviado después con esa huella se
   acepta.
2. Repetir `device add` con el mismo id, o con la huella de otro device, termina con error y la base de
   datos no cambia.
3. Tras `device set-fingerprint`, la huella anterior se rechaza y la nueva se acepta, sin reiniciar el
   server.
4. Tras `device revoke`, toda petición del device se rechaza; sus fixes anteriores siguen guardados.
5. Una huella con `:`, con 63 o 65 caracteres o con caracteres no hexadecimales se rechaza; una huella en
   mayúsculas se acepta y casa con la misma huella en minúsculas enviada por el edge.
6. Ninguna petición HTTP, en ningún listener, crea ni modifica un device.
7. Los tests y ejemplos usan huellas de ejemplo evidentes (p. ej. 64 veces `a`); el repo no contiene
   certificados ni huellas reales.
