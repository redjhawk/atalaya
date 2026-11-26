# SCOPE — Atalaya

Qué es y qué no es el proyecto. Este documento es la **frontera**: cualquier spec, ticket o PR que la
cruce se rechaza. Los agentes lo leen antes de proponer funcionalidad.

## Propósito
Sistema propio y **autoalojado** para **gestionar un device Android rooteado** y recoger
**telemetría de estado**, empezando por la **ubicación**, con un **SOS** por botón físico. Proyecto
**personal de aprendizaje** (Android internals + cliente/servidor Rust). Uso **consentido y conocido**
por la persona que lleva el device.

## Dentro de alcance
- **Ubicación**: fix one-shot y rastreo continuo; histórico; mapa (OpenStreetMap); forzar actualización.
- **Telemetría de estado**: batería, red/cobertura, uso e inventario de apps, integridad, pantalla.
- **Gestión / políticas** (mini-EMM): habilitar/deshabilitar/suspender apps vía `pm`, cola de comandos.
- **SOS**: activación por botón lateral → aviso al grupo (Telegram) + llamada en cascada + captura
  puntual del entorno (foto/audio) disparada por la persona.
- **Infra**: servidor Rust + SQLite, expuesto por Cloudflare Tunnel; app y agente systemizados.
- **Seguridad del canal y del acceso**: ver la sección siguiente; es parte del alcance, no un extra.

## Requisitos de seguridad (condicionan las decisiones técnicas)
- **Solo el device autenticado habla con la API.** El device se autentica con **mTLS** (certificado
  de cliente validado en el edge de Cloudflare) y el server lo identifica por la huella del
  certificado. Mecanismo decidido en `docs/adr/0003-device-mtls-behind-cloudflare.md`.
- **Sin secretos reutilizables en tránsito**: la clave privada se genera en el device, no es
  exportable y no sale de él.
- **El server no se expone directamente**: escucha solo en local y solo es alcanzable por el túnel.
- **El panel web va siempre tras login**, en un hostname distinto al de la API: la ubicación nunca
  queda accesible a quien dé con el hostname.
- **El alta de un device es manual y presencial** (al desplegar); no hay alta por red.
- **Límite conocido**: Cloudflare ve el tráfico en claro. No se persigue cifrado extremo a extremo
  hasta el server.

## Fuera de alcance (no se implementa)
- **Captura o exfiltración de contenido privado**: leer mensajes de apps de mensajería, capturas de
  pantalla en bucle, interceptar tráfico de red, grabar el micro de forma continua o encubierta.
- **Vigilancia de fondo oculta**: toda captura (cámara/audio) es **puntual y disparada por el SOS**,
  nunca silenciosa y permanente.
- **Elusión de cifrado de terceros** o acceso a cuentas ajenas.

## Principio
Atalaya es una herramienta de **gestión y seguridad de un device propio**, equivalente en espíritu a
lo que ofrece Family Link + un botón de pánico. El artefacto no debe poder funcionar como *stalkerware*:
ese es el criterio que decide si algo entra o no.
