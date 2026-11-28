# Setup — pasos manuales

Pasos que solo puede hacer una persona, con acceso al panel de Cloudflare, al T310 o al device. Cada
apartado corresponde a una issue `ready-for-human`.

No se guarda nada sensible en el repo: certificados, claves, hostnames reales y tokens se quedan
fuera. Los ejemplos usan `api-test.tudominio`.

## 1. Comprobar el mTLS en Cloudflare (issue #9)

Confirma que el plan actual de Cloudflare permite lo que exige
`docs/adr/0003-device-mtls-behind-cloudflare.md`: emitir certificados de cliente, exigir mTLS en un
hostname y reenviar la huella del certificado al origen. Bloquea casi todos los tickets del server.

Los nombres de los menús de Cloudflare pueden cambiar; hay que guiarse por el nombre de la función.

### 1.1 Hostname de prueba hacia el T310

En Zero Trust → Networks → Tunnels, abrir el túnel y añadir un *Public Hostname*, por ejemplo
`api-test.tudominio`, apuntando a `http://localhost:8080`.

### 1.2 Crear el certificado de cliente

En el panel del dominio: SSL/TLS → Client Certificates → *Create Certificate*.

- Dejar que Cloudflare genere la clave privada y el CSR. Para la prueba vale; en el device real la
  clave la genera el móvil y no sale de él.
- Guardar el certificado y la clave como `prueba.pem` y `prueba.key`, fuera del repo. La clave solo
  se muestra una vez.

### 1.3 Activar mTLS en el hostname

En la misma página, sección *Hosts*, pulsar *Edit* y añadir `api-test.tudominio`.

### 1.4 Regla que bloquea sin certificado

En la misma página de Client Certificates, pulsar *Create mTLS rule*. Abre una regla WAF ya
rellenada: elegir el hostname `api-test.tudominio` y dejar la acción en *Block*.

Antes de guardar, revisar la expresión que genera. Debe bloquear tanto la falta de certificado
válido como un certificado revocado; si solo comprueba lo primero, editarla hasta que quede así:

```
(http.host eq "api-test.tudominio" and (not cf.tls_client_auth.cert_verified or cf.tls_client_auth.cert_revoked))
```

Sin la parte de `cert_revoked`, el paso 1.9 falla: un certificado revocado seguiría entrando.

La regla también se puede crear a mano en Security → WAF → Custom rules → *Create rule*.

### 1.5 Regla que reenvía la huella

Dentro del dominio (no en la vista de cuenta ni en Zero Trust): Rules → Overview → *Create rule* →
*Request Header Transform Rule*. En paneles antiguos es Rules → Transform Rules → *Modify Request
Header*.

- Cuándo: hostname igual a `api-test.tudominio`.
- Acción: *Set dynamic*, cabecera `X-Client-Cert-Sha256`, valor
  `cf.tls_client_auth.cert_fingerprint_sha256`.

Alternativa si no aparece ese tipo de regla: Rules → Settings → *Managed Transforms* y activar
*Add TLS client auth headers*. Añade varias cabeceras `cf-cert-*`; la de la huella debería ser
`cf-cert-fingerprint-sha256`. Se aplica a todo el dominio, no solo al hostname de prueba.

Sea cual sea la vía, anotar el nombre exacto de la cabecera que llega en el paso 1.7: es el que se
configura en el server.

Si no hay forma de reenviar la huella, no está disponible en el plan y hay que replantear el
ADR 0003.

### 1.6 Escuchar en el T310

```sh
nc -l -p 8080
```

Imprime la petición tal como llega. Según la variante de `nc` puede ser `nc -l 8080`.

`nc` no responde, así que Cloudflare acaba devolviendo un error. Si se prefiere un origen que imprima
las cabeceras y conteste 200:

```sh
python3 -c '
import http.server
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(self.headers)
        self.send_response(200); self.end_headers(); self.wfile.write(b"ok\n")
http.server.HTTPServer(("127.0.0.1", 8080), H).serve_forever()'
```

### 1.7 Probar desde otro equipo

```sh
curl -v --max-time 5 https://api-test.tudominio/
curl -v --max-time 5 --cert prueba.pem --key prueba.key https://api-test.tudominio/
```

- La primera debe dar **403** y no aparecer en `nc`.
- La segunda debe aparecer en `nc` con la cabecera `X-Client-Cert-Sha256`. El `curl` se queda
  esperando o da error porque `nc` no responde; es normal.

Cómo leer el resultado de la segunda petición:

- **403**: Cloudflare no da el certificado por válido. Revisar el paso 1.3 y la regla del 1.4; en
  Security → Events aparece la regla que bloqueó.
- **502**: el certificado ha pasado y el fallo está después, entre el túnel y el origen. No hay nada
  escuchando en el puerto, o `cloudflared` corre en otra máquina o en un contenedor y su `localhost`
  no es el del T310. Probar con `http://127.0.0.1:8080` o con la IP del equipo en la configuración
  del túnel.

### 1.8 Comparar la huella

```sh
openssl x509 -in prueba.pem -noout -fingerprint -sha256 | tr -d ':' | tr 'A-F' 'a-f'
```

Debe coincidir con el valor de la cabecera.

### 1.9 Revocar y repetir

En Client Certificates, revocar el certificado de prueba, volver a lanzar `nc` y repetir la segunda
petición: ahora debe dar **403**.

### 1.10 Resultado

La comprobación está superada si:

- [ ] se pudo crear el certificado y activar mTLS en el hostname,
- [ ] sin certificado la petición recibe 403,
- [ ] con certificado llega al origen con la huella correcta,
- [ ] tras revocar vuelve a recibir 403.

Anotar el resultado y el nombre de la cabecera en el ADR 0003, marcar la casilla de prerrequisitos
del README y cerrar la issue #9. Si algo falla, reabrir el ADR 0003 antes de implementar la
autenticación.

Al terminar, borrar el hostname de prueba y sus reglas, o renombrarlos al hostname definitivo de la
API. Borrar `prueba.pem` y `prueba.key`.

## 2. Poner en marcha la fábrica de agentes

La fábrica (`.factory/`, referencia en `.factory/core/docs/WORKFLOW.md`) necesita dos secretos del
repositorio y un interruptor. Los secretos solo los puede crear el propietario.

### 2.1 Secreto `CLAUDE_CODE_OAUTH_TOKEN`

```sh
claude setup-token
gh secret set CLAUDE_CODE_OAUTH_TOKEN
```

### 2.2 Secreto `FACTORY_TOKEN`

Es un token personal del propietario. La fábrica lo usa para subir ramas, comentar, abrir y fusionar
PR; los agentes nunca lo ven.

1. En GitHub: Settings → Developer settings → Personal access tokens → Fine-grained tokens →
   *Generate new token*.
2. *Repository access*: *Only select repositories*, solo este repositorio.
3. *Repository permissions*, en lectura y escritura: **Contents**, **Issues**, **Pull requests** y
   **Actions**. Los tres primeros los pide la fábrica; *Actions* lo usan el supervisor y la
   reparación para leer los runs y relanzarse.
4. Copiar el valor, que solo se muestra una vez, y guardarlo:

   ```sh
   gh secret set FACTORY_TOKEN
   ```

El token caduca en la fecha elegida al crearlo. Cuando caduque, la fábrica deja de funcionar hasta
renovarlo y volver a guardar el secreto.

### 2.3 Encender y apagar el supervisor

El supervisor lanza la siguiente issue de la cola y reanuda los runs interrumpidos. Está apagado
solo cuando la variable del repositorio `FACTORY_SUPERVISOR` vale exactamente `off`; con cualquier
otro valor, o sin la variable, está encendido.

```sh
gh variable set FACTORY_SUPERVISOR --body off   # apagar
gh variable delete FACTORY_SUPERVISOR           # encender
gh workflow run factory-supervisor.yml          # lanzarlo ya, sin esperar los 15 minutos
```

Otros frenos:

- `gh variable set FACTORY_MAX_ACTIVE --body 0`: no lanza issues nuevas, pero sigue reanudando las
  que ya están en curso.
- Etiqueta `factory-hold` en una issue: el supervisor no la coge. Debe llevarla toda issue que sea
  una spec con sub-issues, que necesite a una persona o que no esté lista.

### 2.4 Lanzar una issue a mano

Poner la etiqueta `ai-dev` a una issue escrita por el propietario. Un comentario del propietario en
esa issue reanuda un run que se paró a preguntar.
