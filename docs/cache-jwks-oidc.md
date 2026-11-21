# Caché de JWKS en el módulo OIDC

## Qué es

Deja de pedir el `jwks_uri` al proveedor de identidad en **cada callback** del flujo OIDC. Antes, `verificar_id_token` llamaba de forma incondicional a `fetch_jwks(&discovery.jwks_uri)`: una petición HTTP por login, aunque la clave del `kid` no hubiera cambiado y el conjunto se acabase de descargar. Se añade una **caché del JWKS** en `AuthState`, análoga a la caché del descubrimiento que ya existía (`discovery_cached`), pero con una diferencia que manda: el conjunto **caduca** (TTL) y además se refresca ante un `kid` desconocido, de modo que una rotación de claves no reinicie el TTL ni obligue a esperar.

Es el follow-up registrado en `docs/autenticacion-oidc.md` y en `HANDOFF.md` («OIDC: caché de JWKS (se pide en cada callback)»). Issue #23; change OpenSpec `cache-jwks-oidc`; capability `autenticacion`, requisito «Caché del JWKS con frescura acotada». Sin cambio del contrato HTTP.

## Ficheros implicados

### Backend (`backend/`)

- `src/auth/state.rs` — se añade `pub jwks: Arc<RwLock<Option<JwksCacheado>>>` a `AuthState` (`:95`), inicializado a `None` en `AuthState::new` (`:117`). `RwLock` y no `Mutex` por la misma razón que el descubrimiento: varios callbacks lo leen a la vez y solo se escribe cuando llega un resultado.
- `src/auth/oidc.rs` — el grueso del cambio:
  - `JWKS_TTL` (`:353`), `Duration::from_secs(10 * 60)`: el techo de frescura de la caché.
  - `JwksCacheado` (`:362`), que empareja el `JwkSet` con su instante de lectura (`leido_en`) y sabe si sigue fresco (`esta_fresco`, `:381`).
  - `jwks_cached(auth)` (`:437`) y su variante con fecha explícita `jwks_cached_en(auth, ahora)` (`:449`): el camino de producción y el que las pruebas pueden caducar.
  - `refrescar_jwks(auth, forzar, ahora)` (`:507`): pide el conjunto y lo deja en la caché, con el candado de escritura sostenido durante la petición.
  - `verificar_id_token` (`:555`) cambia de firma —recibe `auth: &AuthState` en lugar de `settings` + `discovery`— y pasa a resolver las claves por `jwks_cached` (`:560`), con refresco forzado ante `kid` desconocido (`:586`).
- `src/api/auth.rs` — el único llamador de producción del callback (`:168`) se simplifica a `oidc::verificar_id_token(auth, &id_token, &pendiente.nonce)`.
- `src/test_support.rs` — el doble del proveedor gana un contador `peticiones_al_jwks` (`:765`) y un interruptor de caída, para medir cuántas veces se ha ido a por el conjunto y poder fallar el refresco a voluntad.

## Cómo funciona

1. `verificar_id_token` pide las claves por `jwks_cached(auth)`, no por el descubrimiento que el llamador tenga en la mano: quien llama no sabe de dónde sale el conjunto ni cuándo caduca.
2. `jwks_cached_en` consulta primero la frescura bajo un candado de **lectura** —el camino barato, el de todos los callbacks dentro del TTL, que se atienden a la vez sin estorbarse—; solo cuando no hay nada fresco pasa al candado de **escritura** (`refrescar_jwks`).
3. `refrescar_jwks` toma el candado de escritura **antes** de ir a por el conjunto y lo **mantiene durante la petición**. Es lo que impide que dos callbacks que caducan a la vez pidan el conjunto por duplicado: el que llega después espera al candado y ya encuentra la entrada fresca. Con `forzar` a `false` (caducidad) se vuelve a comprobar la frescura ya bajo el candado —de ahí la deduplicación—; con `forzar` a `true` esa comprobación se salta a propósito.
4. Solo se refresca por **caducidad** dentro de `jwks_cached`. El otro motivo para ir al `jwks_uri` —que el ID token venga firmado con un `kid` que el conjunto cacheado no publica— depende del token, no del conjunto, así que lo detecta `verificar_id_token` (`:584`): si `jwks.find(&kid)` falla, fuerza **un solo** refresco (`refrescar_jwks(auth, true, …)`) y reintenta la búsqueda; si tampoco así aparece, rechaza con el `401` de siempre. El camino normal no paga ninguna petición de más.
5. Un fallo de la petición —o del propio descubrimiento— se propaga tal cual y **no se cachea**: se devuelve antes de escribir, así que la entrada anterior se queda como estaba. El proveedor caído da `BadGateway` (`502`) con la envoltura `{error:{code,message}}` y sin sesión; no envenena la caché con un conjunto vacío.
6. El contrato HTTP **no cambia**: un ID token que no verifica sigue siendo `401` genérico (`ID_TOKEN_INVALIDO`, causa solo al log) y el fallo del proveedor al refrescar sigue siendo `502` genérico, sin exponer la causa subyacente.

## Configuración y variables

- Sin variables nuevas. Sin cambios en configuración ni en el despliegue. El TTL es una constante de código (`JWKS_TTL`, 10 minutos), no un ajuste de entorno.

## Cómo habilitar / desplegar

- Nada especial: el cambio va en el binario del backend. Sin migraciones, sin variables y sin tocar el frontend. Aplica al modo `oidc`; en modo deshabilitado (dev en loopback) no entra en juego.

## Decisiones

- **Frescura acotada por TTL, no caché perpetua.** A diferencia de `discovery_cached`, el JWKS no puede servirse indefinidamente: sería seguir aceptando tokens firmados con una clave que el proveedor pudo haber retirado. Los diez minutos son el techo de esa exposición: el mismo orden que el `state` de un login (`STATE_TTL`, cinco minutos) y muy por debajo de la hora que suele vivir un ID token. Ni más corto —volvería a pagar una petición por callback, justo lo que se viene a quitar— ni mucho más largo.
- **Refresco condicional ante `kid` desconocido, además del TTL.** El TTL solo acota el caso que el refresco no puede ver: una clave retirada cuyo `kid` el conjunto cacheado sigue publicando. La rotación normal —clave nueva, `kid` nuevo— la cubre el refresco forzado, que no espera a que venza el TTL. Es una sola pasada de refresco, no un bucle.
- **El instante de lectura va dentro de la entrada** (`JwksCacheado.leido_en`), no en una marca aparte: la caducidad y el conjunto que caduca no pueden separarse sin que una lectura los desempareje (la misma razón por la que el `nonce` viaja junto al `state`).
- **La fecha de evaluación entra por parámetro** (`jwks_cached_en`) en vez de leerse del reloj dentro: deja la comparación determinista en las pruebas sin depender del uptime de la máquina. El camino de producción pasa `Instant::now()`.
- **Candado de escritura sostenido durante la petición.** El precio es que los demás callbacks aguardan mientras dura un refresco —y, en el peor caso, lo que tarde en vencer `INTERCAMBIO_TIMEOUT`—; se acepta porque el refresco solo ocurre al caducar el TTL o al rotar una clave, no en cada callback, y a cambio la caché nunca queda a medias ni se pide dos veces para lo mismo. La **deduplicación vale solo para el refresco por caducidad** (`forzar` a `false`), no para el forzado.
- **La firma de `verificar_id_token` pasa a recibir `&AuthState`.** El conjunto de claves ya no es un documento que el llamador resuelva y pase: se cachea, y `verificar_id_token` es quien sabe cuándo caduca y cuándo refrescar. Alinea su forma con la de `discovery_cached`.

## Follow-ups

- **Deduplicación del refresco forzado entre callbacks concurrentes.** Con `forzar` a `true` (rotación), N callbacks concurrentes con un `kid` desconocido pueden encadenar N peticiones al `jwks_uri`, serializadas bajo el candado pero todas de verdad. Se asume hoy: el caso es raro —solo lo provoca una rotación que varios callbacks pisan a la vez— y el candado ya evita la carrera; una guardia de «refresco en curso» no compensaría la complejidad.
- **`discovery_cached` fallando dentro del refresco.** Si el descubrimiento falla justo cuando `refrescar_jwks` lo necesita, el error se propaga y no se cachea nada, pero ese camino concreto (refresco del JWKS que falla *por el descubrimiento*, no por el `jwks_uri`) no tiene test propio. Candidato a cubrir.
- (Heredados) autorización por rol; logout con cookie inválida; refresh de tokens; chunk de antd >500 kB.
