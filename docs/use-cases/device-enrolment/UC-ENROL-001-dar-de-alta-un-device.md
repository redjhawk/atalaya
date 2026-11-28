# UC-ENROL-001 Dar de alta un device

Especificación: `docs/specifications/device-enrolment/functional.md`.

## Actor

Operador, en la máquina del server.

## Precondiciones

- El operador conoce la huella del certificado de cliente del device (64 caracteres hexadecimales).
- La base de datos de `ATALAYA_DATABASE_PATH` existe o no; si no existe, el subcomando la crea.

## Flujo principal

1. El operador ejecuta `atalaya-server device add <id> <nombre> <huella>`, p. ej.
   `atalaya-server device add phone-1 "Móvil de ejemplo" aaaa…aaaa` (64 veces `a`).
2. El subcomando abre la base de datos de `ATALAYA_DATABASE_PATH`, creándola y migrándola si no existe.
3. El subcomando valida id, nombre y huella según las reglas de entrada.
4. El subcomando guarda el device con ese id, ese nombre y la huella en minúsculas, sin fixes y sin
   último contacto.
5. El subcomando escribe en la salida estándar una línea que confirma el alta e incluye el id, y
   termina con código 0 sin abrir ningún listener.
6. Desde ese momento, un server en marcha sobre la misma base de datos acepta los fixes de ese device
   enviados con esa huella (UC-TELEMETRY-001), sin reiniciarse.

## Flujos alternativos

- **A1 Huella en mayúsculas**: en el paso 1 la huella trae letras `A-F`. Se acepta y se guarda en
  minúsculas; casa con la misma huella en minúsculas reenviada por el edge.
- **A2 Huella revocada**: la huella fue de otro device que ya está revocado. La huella está libre y el
  alta se completa.

## Flujos de error

En todos, el subcomando escribe en la salida de error un mensaje que nombra la causa, termina con código
distinto de 0 y la base de datos no cambia.

- **E1 Id repetido**: ya existe un device con ese id.
- **E2 Huella de otro device**: otro device tiene asignada esa huella.
- **E3 Entrada inválida**: el id incumple las reglas (vacío, más de 64 caracteres, mayúsculas, empieza
  por guion…), el nombre está vacío, es solo espacios o pasa de 100 caracteres, o la huella no tiene
  exactamente 64 caracteres hexadecimales (p. ej. con `:`, con 63 o 65 caracteres).
- **E4 Uso incorrecto**: el número de argumentos no es tres. La salida de error muestra el uso de los
  tres subcomandos y el server no arranca.

## Postcondiciones

- Éxito: el device existe con id, nombre y huella en minúsculas, sin fixes ni último contacto.
- Error: la base de datos queda como estaba.

## Requisitos cubiertos

FR-ENROL-002, FR-ENROL-007, FR-ENROL-008, FR-ENROL-009, FR-ENROL-010, FR-ENROL-011, FR-ENROL-013,
FR-ENROL-014.
