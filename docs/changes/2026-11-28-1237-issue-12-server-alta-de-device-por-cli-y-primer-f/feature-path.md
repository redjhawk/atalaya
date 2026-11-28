# Feature path: alta de device y primer fix autenticado

## 1. Alta y primer fix

1. El operador da de alta un device con un id, un nombre y una huella de ejemplo desde la línea de
   comandos del server. Ve una línea que confirma el alta con ese id.
2. El operador arranca el server. Arranca como antes, con el device API y el panel.
3. El device envía al device API un fix válido para su id, con su huella en la cabecera que reenvía el
   edge. Recibe `201 Created`.
4. El fix aparece guardado para ese device, con su latitud, longitud, precisión, hora y proveedor, y el
   último contacto del device es el momento de esa petición.

## 2. Sustitución de la huella

1. Con el server en marcha, el operador sustituye la huella del device por otra huella de ejemplo. Ve
   una línea que lo confirma.
2. El device envía un fix con la huella nueva. Recibe `201 Created` y el fix queda guardado.
3. Un fix enviado con la huella anterior recibe `401` y no se guarda.

## 3. Revocación

1. Con el server en marcha, el operador revoca la huella del device. Ve una línea que lo confirma.
2. Un fix enviado con la huella que tenía el device recibe `401` y no se guarda; los fixes anteriores
   siguen guardados.
