# UC-ENROL-002 Sustituir la huella de un device

Especificación: `docs/specifications/device-enrolment/functional.md`.

## Actor

Operador, en la máquina del server.

## Precondiciones

- El device existe, con huella o revocado.
- El operador conoce la huella del nuevo certificado del device.
- El server puede estar en marcha sobre la misma base de datos.

## Flujo principal

1. El operador ejecuta `atalaya-server device set-fingerprint <id> <huella>`, p. ej.
   `atalaya-server device set-fingerprint phone-1 bbbb…bbbb` (64 veces `b`).
2. El subcomando abre la base de datos de `ATALAYA_DATABASE_PATH`, creándola y migrándola si no existe.
3. El subcomando valida id y huella según las reglas de entrada.
4. El subcomando sustituye la huella del device por la nueva, en minúsculas. El nombre, los fixes y el
   último contacto no cambian.
5. El subcomando escribe en la salida estándar una línea que confirma la sustitución e incluye el id, y
   termina con código 0 sin abrir ningún listener.
6. En la siguiente petición que recibe el server en marcha, sin reiniciarlo, un fix con la huella nueva
   se acepta y un fix con la huella anterior recibe `401` y no se guarda (UC-TELEMETRY-001).

## Flujos alternativos

- **A1 Device revocado**: el device no tenía huella. Tras el paso 4 vuelve a tener huella y sus
  peticiones con ella se aceptan.
- **A2 Misma huella**: la huella es la que el device ya tiene. El subcomando termina con código 0 y no
  cambia nada.
- **A3 Huella revocada de otro device**: la huella fue de otro device que ya está revocado. Está libre y
  la sustitución se completa.

## Flujos de error

En todos, el subcomando escribe en la salida de error un mensaje que nombra la causa, termina con código
distinto de 0 y la base de datos no cambia.

- **E1 Device inexistente**: no existe ningún device con ese id.
- **E2 Huella de otro device**: otro device tiene asignada esa huella.
- **E3 Entrada inválida**: el id o la huella incumplen las reglas de entrada.
- **E4 Uso incorrecto**: el número de argumentos no es dos. La salida de error muestra el uso de los
  tres subcomandos y el server no arranca.

## Postcondiciones

- Éxito: el device tiene la huella nueva; la anterior ya no identifica a ningún device. Nombre, fixes y
  último contacto siguen iguales.
- Error: la base de datos queda como estaba.

## Requisitos cubiertos

FR-ENROL-003, FR-ENROL-005, FR-ENROL-007, FR-ENROL-008, FR-ENROL-009, FR-ENROL-011, FR-ENROL-012,
FR-ENROL-013, FR-ENROL-014, FR-ENROL-015, FR-TELEMETRY-007.
