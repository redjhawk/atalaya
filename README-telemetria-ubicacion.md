# Atalaya — Telemetría y ubicación de dispositivo (mini-EMM autoalojado)

Sistema propio y autoalojado para **gestionar un dispositivo Android rooteado** y recoger
**telemetría de estado**, empezando por la **ubicación**. Pensado como proyecto personal de
aprendizaje (Android internals + cliente/servidor) y como alternativa a soluciones comerciales
cerradas.

> **Alcance y límite (importante).** Este proyecto cubre **gestión y telemetría de estado del
> dispositivo** (ubicación, uso/inventario de apps, batería, red, integridad) y la **aplicación
> remota de políticas**. **No** cubre la captura ni exfiltración de **contenido privado** del
> usuario del dispositivo (mensajes de apps de mensajería, capturas de pantalla en bucle,
> interceptación de tráfico). La ubicación es estado del dispositivo, equivalente a lo que ofrece
> Family Link. Mantener este límite mantiene el proyecto como herramienta de gestión, no de
> espionaje.

---

## 1. Arquitectura

```
┌──────────────────────── Móvil Android (rooteado, Magisk) ─────────────────────────────┐
│                                                                                        │
│   [App on-device]  ── ubicación (one-shot + continua), escaneo WiFi/celdas,            │
│        │               telemetría, HTTPS + canal WebSocket ──────────────┐            │
│        │ su / socket local (pide acciones root)                          │            │
│        ▼                                                                 │            │
│   [Agente root]  ── acciones root (pm…, dumpsys root, systemize) + persistencia        │
│        │  (módulo Magisk, service.sh)                                    │            │
│        └── escucha botón lateral XCover (getevent) ───► SOS ─────────────┤            │
└──────────────────────────────────────────────────────────────────────────┼────────────┘
                                                                            │ HTTPS + WS
                                                                            ▼
   [Cloudflare]  ──túnel (cloudflared)──►  [T310]  Servidor (Rust/Axum) ──►  SQLite (fichero)
                                                              └──►  /datos/... (ficheros)
```

Principio de diseño: **la app on-device es el cliente** del sistema. Reúne todo lo que necesita las
APIs de Android (ubicación one-shot y continua, escaneo WiFi/celdas, telemetría de sensores/estado,
y su propio **canal WebSocket** con el servidor). El **agente root** es el **ejecutor privilegiado**:
hace lo que requiere `su` (`pm disable-user`, `dumpsys` con root, systemización), garantiza la
**persistencia** (mantiene la app viva y re-concede permisos) y **escucha el botón lateral (SOS)**.
La app pide al agente las acciones root vía `su`/socket local. El **servidor** recibe, persiste y
ofrece el panel y la cola de comandos.

> **Nota de arquitectura (cambio respecto a la idea inicial).** Al principio el agente root era el
> único que hablaba con el servidor y la APK solo escribía un fichero. Como la app ahora necesita red
> de todas formas (rastreo continuo + WebSocket), centralizamos la **red en la app** y dejamos el
> agente **solo para root**. Es más limpio que tener dos clientes de red.

---

## 2. Componentes

### 2.1 App on-device (Android)

App **multipropósito** que agrupa todo lo que necesita APIs de Android (lo que el shell root no hace
bien) y actúa como cliente de red del sistema. **Ya no es "solo ubicación".**

**Funciones**
1. **Ubicación bajo demanda (one-shot):** fix GPS de alta precisión cuando se lo piden (botón
   "actualizar ahora" del panel).
2. **Rastreo continuo:** servicio foreground que registra la traza de forma periódica por sí mismo
   (intervalo adaptativo: más espaciado parado/ahorro de batería, más fino en movimiento), sin
   depender de que el agente lo dispare.
3. **Escaneo del entorno de red:** lista de **APs WiFi** visibles (BSSID/nivel) y **celdas**
   (cellid/señal), para complementar/mejorar la posición y reportar **cobertura real**.
4. **Telemetría que el shell no da bien:** sensores, estado de pantalla (on/off), etc.
5. **Canal WebSocket propio:** mantiene el WS con el servidor, recibe comandos (p. ej.
   `REQUEST_LOCATION`) al instante y envía datos. Para órdenes que requieren root, llama al **agente**
   vía `su`/socket local.

**Stack / decisiones**
- Lenguaje: **Kotlin**.
- Ubicación: **FusedLocationProviderClient** (`play-services-location`):
  `getCurrentLocation(PRIORITY_HIGH_ACCURACY, …)` para el one-shot y `requestLocationUpdates` para el
  rastreo continuo. Fallback a `LocationManager` (AOSP) si Play Services no está.
- Red del entorno: `WifiManager` (escaneo) y `TelephonyManager`/`CellInfo` (celdas).
- Ejecución: **servicio foreground** persistente con `foregroundServiceType="location"` (requisito de
  Android 14), reanudado al arranque y mantenido vivo por el agente.
- WebSocket: cliente (p. ej. OkHttp) con reconexión y backoff.

**Permisos (manifiesto)**
- `ACCESS_FINE_LOCATION`, `ACCESS_COARSE_LOCATION`, `ACCESS_BACKGROUND_LOCATION`
- `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_LOCATION`
- `INTERNET`, `ACCESS_NETWORK_STATE` (ahora sí hace red)
- `ACCESS_WIFI_STATE`, `CHANGE_WIFI_STATE` (escaneo WiFi), `READ_PHONE_STATE` (info de celdas)
- `RECEIVE_BOOT_COMPLETED` (reanudar el servicio tras reinicio)
- `SEND_SMS`, `CALL_PHONE` (solo si el SOS envía SMS / llama en cascada desde el móvil, ver §2.5)
- `CAMERA`, `RECORD_AUDIO` (captura del entorno en el SOS, ver §2.5), con `foregroundServiceType`
  `camera`/`microphone`

**Comportamiento del fix**
1. One-shot: `REQUEST_LOCATION` (por WS o `am start-foreground-service …`) → pide fix → lo envía.
2. Continuo: el servicio acumula fixes y los sube agrupados (ahorro de datos).
3. La app conserva `last.json` como caché local del último fix; el agente puede leerlo con root si
   hace falta.

**Robustez del fix:** `getCurrentLocation` puede devolver `null` o tardar (interior, arranque frío).
Se aplica un **timeout** y, si no llega fix, cae a la **última conocida** / ubicación por red antes de
marcar `no_fix`.

**Formato de salida (`last.json`)**
```json
{ "lat": 48.8566, "lon": 2.3522, "acc": 12.4, "ts": 1732560000000, "prov": "fused" }
```
En caso de no obtener fix: `{ "error": "no_fix" }`.

`acc` = **precisión horizontal estimada en metros** (radio al 68% de confianza,
`Location.getAccuracy()`); el mapa la dibuja como **círculo** alrededor del punto, para distinguir
un fix GPS fino (~10 m) de uno de red (~1-2 km). `prov` = proveedor (gps/fused/network).

**Concesión de permisos con root (sin UI)**
```
pm grant com.jorge.locator android.permission.ACCESS_FINE_LOCATION
pm grant com.jorge.locator android.permission.ACCESS_BACKGROUND_LOCATION
cmd appops set com.jorge.locator android:fine_location allow
dumpsys deviceidle whitelist +com.jorge.locator
```

**Despliegue / persistencia**
- En desarrollo: app normal (`adb install`), para iterar rápido.
- En producción (dispositivo gestionado): **systemizar** vía módulo Magisk
  (`/system/priv-app/`), para que no se pueda desinstalar y persista (misma técnica que se usó
  con Magisk).

### 2.2 Agente (script root, módulo Magisk)

Script que corre como root, lanzado por el `service.sh` de un módulo Magisk (arranca solo, sin ser
una app, no desinstalable por el usuario). **Ya no es el hub de red** (eso es la app, §2.1): es el
**ejecutor privilegiado** y el guardián de persistencia.

**Responsabilidades**
- **Acciones root a petición de la app** (vía `su` / socket local): aplicar políticas
  (`pm disable-user`, `pm suspend`, …), `dumpsys` que requiere root, systemización, y **re-conceder
  permisos** a la app (`pm grant`, `appops`, whitelist de batería) tras updates o reinicios.
- **Persistencia:** vigilar que la app siga viva y **reiniciarla** si el sistema la mata; re-aplicar
  la whitelist de batería; mantener la app/el agente systemizados (anti-desinstalación).
- **Botón SOS:** escuchar el botón lateral XCover con `getevent` y disparar el SOS (ver §2.5).
- **Telemetría root-only:** lo poco que la app no alcanza (`getenforce`, integridad root/adb) se lo
  pasa a la app o lo envía el agente directamente.

**Reparto de red (quién habla con el servidor)**
- **La app** mantiene el **WebSocket** y hace los `POST` de telemetría/ubicación; también lleva la
  **cola offline (store-and-forward)** —IDs generados en cliente para no duplicar tras un ack
  perdido— y la **lógica WiFi/datos** (lo pesado solo en WiFi; fuera, solo ubicación/heartbeat).
- **El agente** solo usa red para casos puntuales (p. ej. avisar de que la app estaba caída). Puede
  llevar un `curl` estático empaquetado para eso.

**Disparo de ubicación (fallback por shell)**
Normalmente el one-shot llega por WS a la app. Como alternativa/arranque, el agente puede forzarlo:
```sh
am start-foreground-service -n com.jorge.locator/.LocationService
sleep 4
cat /data/data/com.jorge.locator/files/last.json   # caché local del último fix
```

**Dependencias en el móvil**
- `getevent`/`toybox` (incluido en Android) para el botón; `busybox` de Magisk para utilidades.
- Opcional: `curl` estático si el agente necesita enviar algo por su cuenta.

### 2.3 Servidor (Rust + SQLite)

API REST que recibe la telemetría, la persiste, y gestiona la cola de comandos del mini-EMM.
La **API es el contrato** (ver endpoints); la implementación de referencia es Rust, pero el agente
y la APK no dependen del lenguaje del servidor.

**Stack**
- **Rust** con **Axum** (sobre Tokio) para el servidor HTTP async.
- **SQLx** como capa de acceso a datos, con **SQLite** (un único fichero `.db`, cero administración).
- **serde** para (de)serializar JSON.
- Binario **estático** (target `x86_64-unknown-linux-musl`) para el T310 (Dell PowerEdge, x86_64):
  se copia y corre, sin runtime ni dependencias.
- Expuesto por **Cloudflare Tunnel** (`cloudflared`), sin abrir puertos.

**Dependencias (Cargo, orientativo)**
```toml
axum = "0.7"
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Endpoints (v1)**
| Método | Ruta | Descripción |
| --- | --- | --- |
| `POST` | `/telemetry/{device}` | Recibe JSON de telemetría (incluida `location`, escaneo, batería…). Persiste. |
| `GET`  | `/commands/{device}` | Comandos pendientes (fallback si el WS está caído). |
| `POST` | `/commands/{device}/ack` | La app confirma la ejecución de un comando. |
| `GET`  | `/locations/{device}?from=&to=` | Histórico de posiciones en un rango (para el mapa). |
| `POST` | `/locations/{device}/refresh` | Empuja `REQUEST_LOCATION` por WebSocket (botón "actualizar ahora"). |
| `POST` | `/sos/{device}` | Recibe un **SOS** (posición + hora); genera aviso destacado. |
| `GET`  | `/ws/{device}` | **WebSocket**: canal de comandos push (servidor ↔ **app**). |
| `GET`  | `/` y estáticos | Sirve la interfaz web (mapa). |

**Modelo de datos (orientativo, SQLx + SQLite)**
- `Device` (id, nombre, token, últimoContacto)
- `LocationFix` (deviceId, lat, lon, acc, ts, recibidoEn)
- `TelemetrySample` (deviceId, batería, red, uso, inventario…, ts)
- `Command` (deviceId, tipo, args, estado: pendiente/enviado/ok/error, creadoEn)

**Payload de telemetría (ejemplo)**
```json
{
  "ts": 1732560000000,
  "location": { "lat": 48.85, "lon": 2.35, "acc": 12.4, "ts": 1732559990000 },
  "battery": { "level": 72, "charging": false, "temp": 28.1 },
  "network": { "wifi": "casa", "mobileData": true, "signal": -67 },
  "wifiScan": [ { "bssid": "aa:bb:…", "level": -54 } ],
  "cells": [ { "cid": 12345, "level": -89 } ],
  "screen": "off",
  "appUsage": [ { "pkg": "com.x", "foregroundMs": 3600000 } ],
  "inventory": { "added": ["com.y"], "removed": [] },
  "integrity": { "root": true, "adb": false, "selinux": "enforcing" }
}
```

**Autenticación**
Decidido en `docs/adr/0003-device-mtls-behind-cloudflare.md`:
- **mTLS en el edge de Cloudflare** para el device: el locator presenta un certificado de cliente
  (clave no exportable en Android Keystore) y Cloudflare bloquea todo lo que no lo traiga.
- **Identidad por huella:** Cloudflare reenvía la huella SHA-256 del certificado y el servidor la
  compara con la registrada para ese `{device}`. Sustituye al Bearer y al service token de Access.
- **Alta manual al desplegar** (un solo device por ahora): CSR generado en el móvil, firmado con la
  CA de cliente de Cloudflare y huella registrada en el servidor.
- **La interfaz web va en otro hostname, tras login** (Cloudflare Access o auth propia): la
  ubicación en vivo no puede quedar expuesta a quien dé con el hostname.

**"Última vez visto" y retención**
- El servidor guarda `últimoContacto` por dispositivo; la UI muestra **hace cuánto** se supo de él y
  su último estado (batería/cobertura).
- **Detección de silencio:** si un dispositivo pasa más de un umbral configurable sin reportar
  (p. ej. 2 h), el servidor lo marca como **"sin señal"** y **avisa** (notificación en la UI;
  opcional correo/push). Sirve para detectar batería agotada, falta de cobertura o manipulación.
- **Retención: 1 día.** Un job purga posiciones y telemetría de más de **24 h**. Mínimo dato
  guardado; suficiente para consulta puntual en caso de necesidad.

### 2.4 Interfaz web (mapa)

Página servida por el propio servidor (Axum sirve ficheros estáticos) para ver la ubicación en un
mapa, sin depender de servicios externos de mapas de pago.

**Requisitos**
- Mapa con **Leaflet** + teselas de **OpenStreetMap** (sin API key).
- **Posición actual:** marcador con la última posición, su precisión (círculo) y la hora.
- **Traza (track):** polilínea uniendo los fixes del periodo en orden temporal.
- **Filtro por día** (selector de fecha, estilo "timeline" de Google Maps): recarga la traza de ese
  día vía `GET /locations/{device}?from=&to=`.
- **Botón "Actualizar ahora"** → `POST /locations/{device}/refresh`: el servidor **empuja** un
  comando `REQUEST_LOCATION` por el **WebSocket** al móvil, que dispara la APK Locator y envía el
  fix fresco; el mapa se refresca. Respuesta casi instantánea.
- (Opcional) auto-refresco del marcador cada X segundos mientras la vista está abierta.
- **Panel de estado del dispositivo** (junto al mapa): **batería** (%, cargando), **cobertura/red**
  (WiFi/datos/sin red, señal) y **"última vez visto"** (hace cuánto llegó el último reporte), con
  aviso visual si lleva mucho en silencio.
- **Avisos de SOS** destacados: cuando llega un `POST /sos`, la UI lo resalta (posición + hora) y,
  opcional, notifica por correo/push.
- (Multi-dispositivo) selector para elegir qué dispositivo ver (tu móvil, el gestionado…).

**Inmediatez del "actualizar ahora":** gracias al **WebSocket** persistente (§2.2) el comando llega
al instante si el móvil tiene red. Si está sin cobertura, el servidor deja el comando pendiente y se
entrega en cuanto el WS reconecta.

### 2.5 Botón SOS / "estoy aquí" (tecla lateral XCover)

El XCover 5 tiene una **tecla lateral programable** (XCover key) y una **Top key**. Se usa para que
la niña pueda enviar un **SOS / "estoy aquí"** con una pulsación, sin abrir nada.

**Implementación recomendada (con root): lo gestiona el agente.**
- El agente lee el **evento crudo** del botón con `getevent -l` sobre el `/dev/input/eventX` que
  corresponde a esa tecla (se identifica pulsándolo y viendo qué evento aparece).
- Al detectar el **patrón de activación** (configurable; por defecto **3 pulsaciones rápidas en
  ~2 s**, ver abajo), dispara el **flujo SOS elegido**:
  1. **fix de ubicación inmediato** (`REQUEST_LOCATION` a la app),
  2. **captura silenciosa del entorno** (ver abajo): foto(s) cámara trasera + frontal, opcional 3-5 s
     de audio,
  3. `POST /sos/{device}` (con ubicación y las fotos) → el servidor avisa al **grupo de Telegram**
     (mensaje + ubicación, incluso en vivo, + **fotos** por `sendPhoto` + botón "ver en mapa"),
  4. **llamada celular en cascada** desde el móvil (contacto 1 → si no contesta en N s, el 2 → …),
  5. (opcional) **notificación local** en el móvil: "SOS enviado".

**Activación (antifalsos)**
- **Por defecto: 3 pulsaciones rápidas en ~2 s.** Evita disparos accidentales (bolsillo, juego) mucho
  mejor que una sola pulsación o un "mantener pulsado", y se hace **a ciegas y discreto** en una
  urgencia real.
- **Vibración corta** de confirmación al lanzarse (la niña sabe que ha funcionado).
- **Ventana de cancelación** configurable (p. ej. 5 s; otra pulsación larga anula) para falsas
  alarmas — o **cero retardo** si se prefiere inmediatez.
- Todo **configurable** (nº de pulsaciones, ventana, retardo), porque leemos el evento crudo con
  `getevent`.

**Captura del entorno en el SOS (foto/audio)**
- La **app** (ya es *foreground service*) hace una **captura silenciosa** con **Camera2/CameraX** vía
  `ImageReader` (**sin previsualización**): cámara **trasera + frontal**; opcional **3-5 s de audio**
  (`MediaRecorder`). Se envían con el `POST /sos` y el bot las reenvía al grupo.
- **El root ayuda:** silenciar el **sonido de obturador** (obligatorio en algunos CSC) y capturar con
  **pantalla apagada**.
- Referencia: Samsung ya trae **"Enviar mensajes SOS"** (triple-power → ubicación + fotos
  frontal/trasera + audio a contactos). Aquí se replica atado a **nuestro botón** y enviado por
  Telegram/servidor (más flexible). Permisos: `CAMERA` (+ `RECORD_AUDIO`), `foregroundServiceType`
  `camera`/`microphone`.
- **Límite:** es captura **puntual disparada por el SOS** (la niña pulsa el botón), para pedir ayuda —
  **no** captura de fondo ni encubierta. Coherente con el alcance del proyecto.
- En el servidor, el SOS genera un **aviso destacado** en el panel (y opcional correo/push).
- Conviene poner la asignación Samsung del botón en **"Ninguna"** (Ajustes → Funciones avanzadas)
  para que solo reaccione nuestro handler.

**Acciones del SOS (a quién avisa y cómo)**
- **WhatsApp llamada de grupo automática: no es viable.** WhatsApp bloquea enviar/llamar por API y
  una llamada grupal no se puede iniciar por programación; automatizar su UI (accessibility) es
  demasiado frágil para una urgencia. No se usa como vía del SOS.
- **En el móvil (sin datos):**
  - **SMS con ubicación** a todo el grupo a la vez (`SmsManager`). Botón de pánico clásico: funciona
    con cobertura mínima.
  - **Llamada en cascada** (`CALL_PHONE`): contacto 1; si no contesta, el 2… (conferencia a varios a
    la vez no se automatiza; cascada sí).
- **Fan-out en el servidor (recomendado para "grupo"):** el `POST /sos` dispara el reparto:
  - **Telegram (bot a un grupo):** gratis y robusto; un `POST` y el aviso + ubicación (incluso **en
    vivo**) + botón "ver en mapa" aparece en el grupo al instante. **Vía recomendada para avisar.**
    Ojo: los **bots solo mandan mensajes, no llaman**. Las llamadas de Telegram existen pero solo
    desde **cuenta de usuario** (TDLib/MTProto, no bot) y una voice chat de grupo **no suena** a los
    miembros (es una sala a la que se unen). Para "que suene un teléfono" → llamada celular o Twilio.
  - **WhatsApp Business/Cloud API:** solo **textos** de aviso (no llamadas), con cuenta business,
    plantillas aprobadas y coste. Para familia, Telegram es más simple.
  - **Email / push**, y opcional **llamada de voz automática** vía proveedor (p. ej. Twilio, puede
    marcar a varios).
- **Flujo elegido:** **mensaje al grupo de Telegram** (bot, con ubicación en vivo) **+ llamada
  celular en cascada** al grupo desde el móvil. El Telegram avisa a todos a la vez; la llamada
  garantiza la voz aunque tengan el móvil en silencio para apps. (SMS y email quedan como refuerzos
  opcionales.)

**Alternativas**
- **Sin código:** asignar la tecla (Ajustes → Funciones avanzadas) a **lanzar una mini-app** que, al
  abrirse, dispara el SOS y se cierra. Simple, pero depende de que el sistema lance la app.
- **App sin root:** captar el botón en segundo plano exige *accessibility service* o remap de Knox;
  más frágil. Estando rooteado, mejor la vía del agente.

**Decisión:** el **disparo** del SOS lo hace el **agente root** (robusto, siempre activo); una
**mini-app aparte** solo si se quiere pantalla de confirmación visible para la niña.

**Blindaje del botón (evitar que se reconfigure)**
- Clave: al leer el **evento crudo** con `getevent`, la config de Samsung **no afecta** al SOS —
  pase lo que pase en Ajustes, el demonio ve la pulsación. Blindar solo evita que el botón, además,
  lance otra cosa o se repurpose.
- **(a) Remap en el keylayout (overlay Magisk, recomendado):** identificar el *scancode* del botón
  con `getevent -l` y remapearlo en el `.kl` (`/system/usr/keylayout/…`) vía overlay *systemless* de
  Magisk a un keycode neutro/propio. Android deja de tratarlo como la "tecla XCover" configurable →
  la opción de Ajustes queda sin efecto; el demonio sigue leyendo el scancode crudo. Reversible
  (quitar el módulo). **Backup del `.kl` antes de tocar.**
- **(b) Vigilante del ajuste:** forzar la asignación a "Ninguna" con `settings put …` y que el agente
  la **resetee** si cambia (patrón de persistencia ya usado). Requiere localizar la clave exacta en el
  móvil (`settings list system/secure/global`, comparando antes/después de cambiar el ajuste).
- Sin Device Owner (Knox quemado) no hay vía "oficial" de políticas de tecla; por eso se hace por
  root.

---

## 3. Despliegue (paso a paso)

### Prerrequisitos (comprobar antes de lanzar el proyecto)

La autenticación por mTLS (`docs/adr/0003-device-mtls-behind-cloudflare.md`) depende de dos cosas
que hay que verificar con una prueba real antes de empezar a implementar:

- [ ] **Cloudflare: CA de cliente y reenvío de la huella en el plan actual.**
  1. *SSL/TLS → Client Certificates*: se puede crear un certificado y activar mTLS en el hostname.
  2. *Rules → Transform Rules*: una regla añade la cabecera `X-Client-Cert-Sha256` con
     `cf.tls_client_auth.cert_fingerprint_sha256`.
  3. *WAF → Custom rules*: bloquear si `not cf.tls_client_auth.cert_verified or
     cf.tls_client_auth.cert_revoked`.
  4. Prueba: con `nc -l -p 8080` tras el túnel, `curl` sin certificado da 403 y `curl --cert/--key`
     llega al origen con la cabecera; su valor coincide con
     `openssl x509 -noout -fingerprint -sha256`. Tras revocar el certificado, vuelve a dar 403.
- [ ] **XCover 5: Keystore respaldado por hardware con Knox quemado.**
  1. `adb shell pm list features | grep -i keystore` muestra `android.hardware.hardware_keystore`.
  2. Una app de prueba genera una clave EC en `AndroidKeyStore` y `KeyInfo.securityLevel` devuelve
     `TRUSTED_ENVIRONMENT` o `STRONGBOX` (no `SOFTWARE`).
  3. Una firma de prueba con esa clave funciona.

Si falla la primera, hay que replantear el ADR 0003. Si falla la segunda, el mTLS sigue siendo
viable, pero root podría copiar la clave y hay que anotarlo en el ADR.

### Servidor (T310)
1. Compilar el binario Rust estático:
   `cargo build --release --target x86_64-unknown-linux-musl`.
   Las migraciones de SQLx crean/migran el fichero SQLite al arrancar (o con `sqlx migrate run`).
2. Copiar el binario + el fichero `.db` al T310 y arrancarlo (escucha en `localhost:8080`).
   Recomendado: unidad **systemd** para que arranque solo y se reinicie si cae.
3. Instalar y configurar **cloudflared**: túnel desde `https://<host>.<dominio>` →
   `http://localhost:8080`. Sin abrir puertos en el router.
4. Activar **mTLS** en el hostname de la API (regla WAF que bloquea sin certificado válido) y poner
   el hostname del panel tras **Cloudflare Access**.

### Móvil
1. Compilar la **APK Locator** (Android Studio / Gradle) e instalarla.
2. Conceder permisos con root (ver §2.1) y exonerar de batería.
3. **Alta del certificado** (por `adb`): la app genera la clave y el CSR, se firma con la CA de
   cliente de Cloudflare, se devuelve el certificado a la app y se registra su huella en el servidor.
4. Instalar el **módulo Magisk del agente** (`service.sh` + script + configuración).
5. (Producción) **Systemizar** la APK Locator y el agente para que no se puedan quitar.
6. Reiniciar y verificar que el agente envía telemetría y ubicación al servidor.

---

## 4. Referencia: comandos de telemetría (root/shell)

| Dato | Comando |
| --- | --- |
| Batería/estado | `dumpsys battery` |
| Uso de apps | `dumpsys usagestats` / `cmd usagestats` |
| App en primer plano | `dumpsys activity activities \| grep mResumed` |
| WiFi / red | `dumpsys wifi`, `dumpsys telephony.registry` |
| Consumo de datos por app | `dumpsys netstats` |
| Almacenamiento / RAM | `df /data`, `dumpsys meminfo` |
| Inventario de apps | `pm list packages -f --show-versioncode` |
| Integridad (root/adb/selinux) | `su -c id`, `settings get global adb_enabled`, `getenforce` |
| Último arranque | `cat /proc/uptime` |
| Versión / parche | `getprop ro.build.version.release`, `…security_patch` |
| Ubicación (última conocida) | `dumpsys location` (fix fresco: APK Locator) |

---

## 5. Roadmap

- **Fase 1 — Ubicación (en curso):** app on-device (ubicación one-shot) que la obtiene y **envía**
  (HTTPS + WebSocket) + endpoint `POST /telemetry` con `location` + **histórico** persistido.
  **Interfaz web con mapa OSM (Leaflet):** posición actual, **traza**, **filtro por día** y botón
  **"actualizar ahora"** (`REQUEST_LOCATION` **vía WebSocket**). Incluye **cola offline** (en la app),
  **fallback de GPS**, **batería/cobertura** en cada reporte, **UI tras login** y **retención 1 día**.
- **Fase 1.5 — App multipropósito:** **rastreo continuo** (traza automática), **escaneo WiFi/celdas**
  (cobertura + ayuda a la posición) y **botón SOS** (tecla lateral XCover vía agente root).
- **Fase 2 — Telemetría de estado:** batería, uso de apps, consumo de datos, inventario con
  altas/bajas, integridad, estado de pantalla.
- **Fase 3 — Comandos remotos (mini-EMM):** cola de comandos + ejecución en el agente
  (`pm disable-user`, `pm suspend`, …) + `ack`.
- **Fase 4 — Robustez y operación:** detección de silencio + **alertas**, **purga automática**
  (retención 1 día), **backup del servidor** (fichero SQLite), observabilidad/logs, y evolución del
  push a **MQTT** si se quiere entrega garantizada offline (QoS, sesión persistente, LWT).

> Nota: la **cola offline**, la **UI tras login** y la **autenticación por dispositivo** se adelantan
> a la Fase 1 (son críticas), no esperan a la Fase 4.

---

## 6. Contexto del dispositivo de referencia

- Galaxy XCover 5 (SM-G525F), Android 14, **rooteado con Magisk** (bootloader desbloqueado,
  Knox quemado → Device Owner no disponible; de ahí el enfoque basado en **root** en lugar de
  DevicePolicyManager).
- Servidor: **T310** autoalojado (Dell PowerEdge, x86_64), expuesto por **Cloudflare Tunnel**.

---

## 7. Decisiones de diseño (resumen)

- **Kotlin** para la app (idioma nativo de Android, documentación first-class).
- **App multipropósito como cliente de red; agente solo para root.** La app necesita red igualmente
  (rastreo continuo + WebSocket), así que centraliza red/WS/cola offline; el agente se queda con lo
  que exige `su` y con la persistencia. Evita tener dos clientes de red. (Cambio respecto al diseño
  inicial, donde el agente era el hub y la APK solo escribía un fichero.)
- **App normal en desarrollo; systemizada en despliegue** (anti-desinstalación).
- **SOS por el botón lateral vía agente root (`getevent`)**: siempre activo, sin UI, no se salta;
  mini-app de confirmación solo si se quiere feedback visible.
- **Servidor en Rust (Axum) + SQLite:** un binario estático y un fichero de BD → despliegue y
  backup triviales en el T310. Elegido también como lenguaje de aprendizaje. La API es un contrato
  agnóstico: el resto del sistema no depende de esta elección.
- **Canal de comandos por WebSocket** (Axum nativo, sin componentes extra): push instantáneo del
  "actualizar ahora" con gasto de datos mínimo frente al sondeo. **MQTT (broker)** queda como
  evolución si se quiere entrega garantizada offline (QoS, sesión persistente, LWT). Cloudflare
  Tunnel soporta WebSocket.
- **Retención de 1 día + UI tras login:** enfoque personal/emergencia, mínimo dato guardado, acceso
  siempre protegido.
- **Límite de alcance:** telemetría de estado y gestión, no captura de contenido privado.

---

## 8. Flujo de desarrollo con *skills* (factoría de agentes)

El proyecto se construye con una **factoría de agentes**: cualquier agente de código (Claude Code,
Codex, Copilot, Gemini, Cursor…) trabaja sobre el repo con el **mismo conjunto de *skills*** y las
mismas convenciones, para que la disciplina (specs, TDD, revisión) sea idéntica desde el primer
commit. Se usan los *skills* de **[`mattpocock/skills`](https://github.com/mattpocock/skills)** (MIT):
flujos reutilizables definidos por `SKILL.md`, unos **invocados por el usuario** (slash commands) y
otros **invocados por el modelo**.

### 8.1 Bootstrap (antes de escribir código)
- **Claude Code:** `claude plugin install mattpocock-skills@claude-plugins-official`.
- **Otros agentes:** `npx skills@latest add mattpocock/skills -a <agente>` (cursor, codex, copilot,
  gemini, opencode, windsurf…); actualizar con `npx skills@latest update`. **No** instalar el plugin
  *y* los ficheros copiados a la vez (se cargarían dos veces).
- Ejecutar **una vez por repo** `/setup-matt-pocock-skills` (pregunta el issue tracker, las labels de
  triage y dónde guardar los docs).
- (Comandos exactos: verificar en el README del repo, que evoluciona.)

### 8.2 Estructura del repo (monorepo: un componente = un sub-proyecto)
```
/apps/locator      # app Kotlin on-device (§2.1)
/agent             # módulo Magisk + scripts root (§2.2, §2.5)
/server            # Rust/Axum + SQLite (§2.3, §2.4)
/docs              # specs y tickets generados por los skills
AGENTS.md          # instrucciones comunes a todos los agentes
CLAUDE.md          # idem, específico de Claude Code
GLOSSARY.md        # vocabulario compartido (evita ambigüedad/verbosidad)
SCOPE.md           # frontera del proyecto: qué entra y qué no
```
Nombre del repo: **`atalaya`**. Componentes: **`atalaya-locator`** (app), **`atalaya-agent`** (módulo
root), **`atalaya-server`** (Rust). La **API es el contrato** (§2.3): cada componente lo puede
construir un agente distinto **en paralelo** sin pisarse, mientras respeten los endpoints.

### 8.3 Pipeline (de la idea al PR)
1. **Alinear:** `/grill-me` o `/grill-with-docs` → el agente hace preguntas antes de tocar nada.
2. **Especificar:** `to-spec` → convierte este README + respuestas en una **spec** por componente.
3. **Trocear:** `to-tickets` → divide la spec en **tickets** pequeños.
4. **Implementar:** `implement-spec` / `implement` → ticket a ticket.
5. **Correctitud:** `tdd` (red-green-refactor) y `diagnosing-bugs` como bucles de feedback.
6. **Entregar:** `code-review` + `pr` para revisar y abrir el PR.
7. **Mantener:** `improve-codebase-architecture` (sugiere módulos más simples) y `retro` (aprender del
   proceso).

### 8.4 La "factoría" en la práctica
- **Un agente por componente** (locator / agente / server), todos compartiendo `AGENTS.md` +
  `GLOSSARY.md` + los skills → misma disciplina, distinto dominio.
- Los skills son **agnósticos de lenguaje**: estructuran el trabajo (specs, TDD, revisión), pero el
  conocimiento Android/Rust lo aporta el agente. Encajan con Kotlin y Rust por igual.
- `GLOSSARY.md` fija los términos para que todos los agentes hablen igual.

### 8.5 Semilla de `GLOSSARY.md`
- **device**: móvil gestionado (XCover 5).
- **locator**: la app on-device (§2.1).
- **agente**: proceso root (módulo Magisk): ejecutor + persistencia + SOS (§2.2, §2.5).
- **fix**: una lectura de ubicación (`lat/lon/acc/ts/prov`).
- **acc**: precisión horizontal en metros (68 % de confianza).
- **SOS**: flujo de alarma (activación por botón → Telegram + llamada en cascada + captura) (§2.5).
- **telemetría**: estado del dispositivo (batería, red, inventario, integridad…).
- **comando / ack**: orden del servidor al device y su confirmación.
