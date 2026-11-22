# Deduplicar el refresco forzado del JWKS

## Qué es

Evita que N callbacks concurrentes con un `kid` rotado paguen N peticiones al `jwks_uri`. El refresco por caducidad ya se deduplicaba bajo el candado de escritura; el **forzado** por rotación —cuando el ID token trae un `kid` que el conjunto cacheado no publica— no lo hacía: cada callback que detectaba el `kid` ausente volvía a pedir el conjunto, serializado bajo el candado pero con una petición real cada vez.

Ahora el refresco forzado reutiliza el conjunto que ya refrescó otro callback. La marca de lectura de la caché (`JwksCacheado.leido_en`) es el testigo: si la entrada actual se leyó después de la lectura que observó el llamador, su conjunto ya incorpora el refresco que este iba a pedir y no hace falta ir al proveedor.

- Issue: #41
- Change OpenSpec: `dedupe-jwks-rotacion`
- Plan: `plans/PLAN-020.md`

## Ficheros implicados

### Backend (`backend/`)

- `src/auth/oidc.rs` — el grueso del cambio:
  - `jwks_cached_con_marca` (`:471`) devuelve `(JwkSet, Instant)`: el conjunto y la marca de lectura con la que quedó cacheado.
  - `refrescar_jwks` (`:531`) pierde el parámetro `forzar` y devuelve `(JwkSet, Instant)`. Sigue siendo el camino del refresco por **caducidad**.
  - `refrescar_bajo_candado` (`:551`) hace el `fetch` con el candado ya tomado y devuelve el conjunto junto con la marca escrita.
  - `refrescar_jwks_si_obsoleto` (`:586`) es el camino del refresco **forzado**: compara la marca observada con la de la entrada actual y reutiliza el conjunto si otro callback ya refrescó.
  - `verificar_id_token` (`:629`) resuelve las claves por `jwks_cached_con_marca` y pasa la marca al refresco forzado.
- `src/test_support.rs` — el proveedor falso ya contaba las lecturas del `jwks_uri` (`peticiones_al_jwks`), permite rotar la clave (`rotar_clave` / `firmar_con_rotada`) y retrasar la respuesta (`retrasar_jwks`); con eso se reproducen la rotación y la concurrencia.
- `docs/cache-jwks-oidc.md` — se actualiza el comportamiento y se retira el follow-up pendiente.

## Cómo funciona

1. `verificar_id_token` pide las claves por `jwks_cached_con_marca(auth)`. La lectura devuelve el conjunto y la marca `leido_en` de la entrada cacheada.
2. Si el `kid` del ID token está en el conjunto, no se toca el `jwks_uri`: el camino normal no paga ninguna petición de más.
3. Si el `kid` no está, se llama a `refrescar_jwks_si_obsoleto(auth, visto_en, ahora)` con la marca de la lectura inicial.
4. Bajo el candado de escritura —el mismo que serializa todos los refrescos—, si la entrada actual tiene `leido_en > visto_en`, otro callback ya refrescó después de la lectura observada: se devuelve su conjunto sin pedir nada.
5. Si la marca no avanzó, se delega en `refrescar_bajo_candado`, que hace el descubrimiento, pide el `jwks_uri` y escribe la entrada con el `ahora` recibido. El `fetch` queda dentro de la sección crítica.
6. `refrescar_jwks` y `refrescar_bajo_candado` devuelven `(JwkSet, Instant)`. La marca que se entrega es la que se acaba de escribir, no una relectura posterior de la caché: así nunca se devuelve una marca más nueva que el conjunto entregado.
7. Un fallo del refresco se propaga tal cual y **no se cachea**: la entrada anterior se queda como estaba. El contrato HTTP no cambia: `401` genérico para un ID token que no verifica y `502` `BadGateway` para un fallo del proveedor, sin exponer la causa.

## Configuración y variables

- Sin variables nuevas. Sin cambios en configuración ni en el despliegue. El TTL sigue siendo la constante de código `JWKS_TTL` (10 minutos).

## Cómo habilitar / desplegar

- Nada especial: el cambio va en el binario del backend. Sin migraciones, sin variables y sin tocar el frontend. Aplica al modo `oidc`; en modo deshabilitado (dev en loopback) no entra en juego.

## Decisiones

- **La marca de lectura viaja dentro de la entrada** (`JwksCacheado.leido_en`), no en una estructura aparte: la caducidad y el conjunto que caduca no pueden desemparejarse sin que una lectura los separe. La misma marca sirve para la frescura y para deduplicar el forzado.
- **El refresco devuelve su propia marca.** `refrescar_jwks` y `refrescar_bajo_candado` devuelven `(JwkSet, Instant)` en vez de releer la caché después. Una relectura podía devolver una marca más nueva que el conjunto entregado —si otra tarea refrescaba entre la escritura y la lectura— y romper la comparación del siguiente callback.
- **Se elimina el parámetro `forzar`.** `refrescar_jwks` solo atiende la caducidad; el refresco forzado vive en `refrescar_jwks_si_obsoleto`, que es donde tiene sentido comparar la marca. Un booleano que siempre llegaba a `false` era superficie muerta y hacía más difícil leer el flujo.
- **Comparación estricta `leido_en > visto_en`.** Basta con que la entrada se haya leído después de la marca observada. No se compara el contenido del conjunto: el testigo es el instante, no la clave, porque el refresco puede devolver un conjunto distinto aunque el `kid` siga ausente.
- **Sin guardia de «refresco en curso».** El candado de escritura ya serializa los refrescos y la marca resuelve la deduplicación; una guardia adicional añadiría estado y complejidad sin cubrir un caso que la marca no cubra.

## Riesgos y follow-ups

- **La garantía es *best-effort* ante carreras estrechas.** El candado evita la mayoría de los casos, pero si el refresco falla el siguiente callback puede reintentarlo; y dos marcas que el reloj no distinga podrían no deduplicarse. Como mucho se paga una petición de más; nunca se sirve un conjunto incorrecto.
- **Fallo del proveedor durante el refresco.** La caché no se envenena: el conjunto anterior sigue sirviendo. Está cubierto por el test del `502`, que después verifica un token con el `kid` original sin aumentar el conteo de peticiones.
- **`discovery_cached` fallando dentro del refresco.** El error se propaga y no se cachea nada; la caché anterior del JWKS se conserva. Está cubierto por el change `discovery-caido-en-refresco-jwks` (ver `docs/discovery-caido-en-refresco-jwks.md`).
- **Rotación con muchos callbacks.** Con muchos callbacks a la vez, el primero paga el `fetch` y los demás esperan al candado; el coste es el tiempo de la petición, no peticiones adicionales. Si el proveedor tarda, el `INTERCAMBIO_TIMEOUT` acota la espera.
