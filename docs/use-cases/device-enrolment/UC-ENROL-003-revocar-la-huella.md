# UC-ENROL-003 Revocar la huella de un device

Especificación: `docs/specifications/device-enrolment/functional.md`.

## Actor

Operador, en la máquina del server.

## Precondiciones

- El device existe.
- El server puede estar en marcha sobre la misma base de datos.

## Flujo principal

1. El operador ejecuta `atalaya-server device revoke <id>`, p. ej. `atalaya-server device revoke phone-1`.
2. El subcomando abre la base de datos de `ATALAYA_DATABASE_PATH`, creándola y migrándola si no existe.
3. El subcomando valida el id según las reglas de entrada.
4. El subcomando deja el device sin huella. El nombre, los fixes y el último contacto no cambian.
5. El subcomando escribe en la salida estándar una línea que confirma la revocación e incluye el id, y
   termina con código 0 sin abrir ningún listener.
6. En la siguiente petición que recibe el server en marcha, sin reiniciarlo, toda petición de ese device
   recibe `401` y no se guarda, sea cual sea la huella (UC-TELEMETRY-001). Sus fixes anteriores siguen
   guardados.

## Flujos alternativos

- **A1 Ya revocado**: el device ya no tenía huella. El subcomando confirma con código 0 y no cambia nada.
- **A2 Reasignar**: más tarde, el operador asigna una huella al device con `set-fingerprint`
  (UC-ENROL-002) y sus peticiones con esa huella vuelven a aceptarse.
- **A3 Huella liberada**: la huella revocada queda libre y el operador puede asignarla a otro device
  (UC-ENROL-001, UC-ENROL-002).

## Flujos de error

En todos, el subcomando escribe en la salida de error un mensaje que nombra la causa, termina con código
distinto de 0 y la base de datos no cambia.

- **E1 Device inexistente**: no existe ningún device con ese id.
- **E2 Entrada inválida**: el id incumple las reglas de entrada.
- **E3 Uso incorrecto**: el número de argumentos no es uno. La salida de error muestra el uso de los
  tres subcomandos y el server no arranca.

## Postcondiciones

- Éxito: el device existe sin huella; ninguna petición suya se acepta. Nombre, fixes y último contacto
  siguen iguales.
- Error: la base de datos queda como estaba.

## Requisitos cubiertos

FR-ENROL-004, FR-ENROL-006, FR-ENROL-007, FR-ENROL-008, FR-ENROL-009, FR-ENROL-011, FR-ENROL-012,
FR-ENROL-013, FR-ENROL-014, FR-TELEMETRY-007.
