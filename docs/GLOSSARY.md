# GLOSSARY — Atalaya

Vocabulario compartido del proyecto. Todos los agentes y documentos usan **estos** términos para
evitar ambigüedad y verbosidad. Si un concepto no está aquí, se añade antes de usarlo en el código.

## Componentes
- **device**: el móvil gestionado; por ahora uno solo (Samsung Galaxy XCover 5, rooteado con Magisk).
- **atalaya-locator** (*locator*): app Kotlin on-device. Ubicación (one-shot + continua), escaneo
  WiFi/celdas, telemetría de sensores/pantalla, canal WebSocket y captura del SOS.
- **atalaya-agent** (*agente*, *agent*): proceso root (módulo Magisk). Ejecutor privilegiado (`pm`, `dumpsys`
  root, systemize), persistencia (mantiene vivo el locator) y escucha del botón lateral (SOS).
- **atalaya-server** (*server*): servidor Rust (Axum) + SQLite. Recibe telemetría, persiste, sirve el
  mapa y gestiona la cola de comandos y el fan-out del SOS.

## Datos
- **fix**: una lectura de ubicación. Campos: `lat`, `lon`, `acc`, `ts`, `prov`.
- **acc**: precisión horizontal estimada en metros (radio al 68 % de confianza).
- **prov**: proveedor del fix (`gps` / `fused` / `network`).
- **telemetría** (*telemetry*): estado del device (batería, red/cobertura, uso/inventario de apps, integridad,
  pantalla). **Estado, no contenido privado.**
- **track / traza**: secuencia ordenada de fixes que dibuja el recorrido.
- **última vez visto**: `últimoContacto`, cuándo se supo por última vez del device.

## Identidad y acceso
- **operador** (*operator*): la persona que administra el server y despliega el device. Evitar
  *admin*, *usuario*.
- **huella** (*fingerprint*): SHA-256 del certificado de cliente del device, 64 caracteres hex en
  minúsculas; identifica al device (ADR 0003). No es un secreto. Evitar *token*, *clave*, *hash*.
- **alta** (*enrolment*): registro manual de un device en el server con id, nombre y huella, por línea
  de comandos. Evitar *registro*, *signup*, *provisioning*.
- **revocar** (*revoke*): dejar un device sin huella; sus peticiones se rechazan hasta asignarle otra.
- **device API**: listener del server para el device, tras mTLS en el edge; el único donde se confía
  en la cabecera de huella.
- **panel**: listener del server para la interfaz web, tras login interactivo. Evitar *dashboard*,
  *admin*.

## Protocolo
- **comando** (*command*): orden del server al device (p. ej. `REQUEST_LOCATION`, `pm disable-user`).
- **ack**: confirmación del device de que recibió o completó un comando.
- **cola offline**: reportes guardados en el device cuando no hay red, reenviados al reconectar
  (store-and-forward, con IDs de cliente para no duplicar).
- **heartbeat**: señal ligera periódica para saber que el device sigue vivo.

## SOS
- **SOS**: flujo de alarma disparado por el botón físico del device, que reporta la ubicación y avisa
  al grupo configurado.
- **activación**: patrón del botón que lanza el SOS (por defecto 3 pulsaciones rápidas en ~2 s).
- **fan-out**: reparto del aviso de SOS desde el server (Telegram, llamada, email…).
- **cascada**: llamada telefónica secuencial a los contactos (1 → si no contesta, 2 → …).
- **captura del entorno**: foto(s) silenciosas (trasera + frontal) y audio opcional al disparar el SOS.

## Factoría
- **task**: tipo reutilizable de trabajo de la factoría (p. ej. revisión o implementación).
- **trigger**: evento que inicia un run.
- **run**: una ejecución de una task en un runner desechable.
- **runner**: máquina desechable que ejecuta un run.
- **ticket tracker**: GitHub Issues, la interfaz humana de la factoría.
- **git host**: GitHub, donde viven ramas, pull requests y revisiones.
- **orchestrator**: GitHub Actions, que recibe los triggers e inicia los runs.
