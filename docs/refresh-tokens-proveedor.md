# Refresh del ID token del proveedor

## Qué es

Capturar y usar el `refresh_token` emitido por el proveedor OIDC para renovar el `id_token` de la sesión local, manteniendo la vida deslizante y la seguridad del flujo.

## Ficheros implicados

- `backend/src/auth/oidc.rs`: `canjear_codigo` devuelve el `refresh_token` (`Canje`); `refrescar_id_token` lo envía al `token_endpoint` con `grant_type=refresh_token`; `verificar_id_token_renovado` verifica el ID token del refresh **sin** `nonce`.
- `backend/src/auth/session.rs`: `Claims` lleva `jti: Option<String>`; `issue_con_jti` firma con `jti`; `renew` lo conserva; `nuevo_jti` lo genera.
- `backend/src/auth/state.rs`: `AuthState` mantiene `refresh_tokens` en memoria (`jti → (token, último uso)`, acotado por TTL), un *tombstone* de `jti` revocados y un candado por `jti` para serializar el refresh; métodos `obtener_refresh_token_por_jti`, `guardar_refresh_token`, `rotar_refresh_token`, `borrar_refresh_token`, `revocar_jti`, `esta_revocado` y `cerrojo_de_refresh`.
- `backend/src/auth/middleware.rs`: el *gate* detecta `jti` + `refresh_token`, pide el ID token nuevo al proveedor **serializado por `jti`** (una sola petición a la vez), contrasta la identidad del ID token nuevo con la de la sesión, rota el token si el proveedor lo emite y cierra la sesión (`401` + cookie borrada) ante `invalid_grant`, marcando el `jti` para que la revocación sea **pegajosa**.
- `backend/src/api/auth.rs`: el callback emite `jti` y asocia el `refresh_token`.
- `backend/src/auth/oidc.rs` (tests) y `backend/src/test_support.rs` (`ProveedorFalso`): cubren captura, renovación, rotación, `invalid_grant`, proveedor caído y sesión sin `refresh_token`.
- `docs/refresh-tokens-proveedor.md`: descripción del flujo y criterios de aceptación.
- `docs/autenticacion-oidc.md`: actualización del follow-up para indicar que el refresh de tokens es parte del flujo.

## Flujo de trabajo

1. **Captura**: en el callback OIDC, si el `token_endpoint` devuelve un `refresh_token`, se captura.
2. **Asociación**: la sesión emitida lleva `jti`; el `refresh_token` se guarda en memoria bajo `jti`.
3. **Renovación**: en la ventana de renovación (≤ 2 h), si hay `jti` y `refresh_token`, se llama al `token_endpoint` con `grant_type=refresh_token`, se verifica el nuevo `id_token` y se emite la cookie con el mismo `jti`. La petición va **serializada por `jti`**: dos peticiones concurrentes de la misma sesión no piden el refresh a la vez.
4. **Rotación**: el proveedor puede enviar un `refresh_token` nuevo; se sustituye el anterior en memoria.
5. **Rechazo**: si el proveedor responde `invalid_grant`, se descarta el `refresh_token`, se marca el `jti` como revocado y se devuelve `401` con cookie de sesión borrada. La revocación es pegajosa: las peticiones siguientes con la misma cookie siguen cerrando la sesión.
6. **Sin refresh**: si no hay `refresh_token`, la renovación local sigue igual que antes.

## Criterios de aceptación

- [x] El `refresh_token` se captura y asocia a `jti` en el callback.
- [x] La renovación deslizante usa `refresh_token` para solicitar `id_token` nuevo.
- [x] Un `invalid_grant` cierra la sesión y borra la cookie.
- [x] Sin `refresh_token`, la renovación local es idéntica al comportamiento actual.
- [x] `just check-all` verde.

## Variables y configuración

- `PODPOT_OIDC_CLIENT_SECRET` autentica a podpot contra el `token_endpoint` al canjear el código y al refrescar; no firma el `refresh_token` —eso lo hace el proveedor—.
- `AuthMode::Oidc` permite el flujo; `Disabled` lo bloquea.
- `AuthState::refresh_tokens` vive en memoria durante la vida del proceso y se acota por TTL: al insertar se purgan las entradas sin tocar en `SESSION_LIFETIME`. Se pierde al reiniciar.

## Notas

- El `refresh_token` se pierde al reiniciar el proceso; la sesión local sigue válida pero sin renovación del proveedor.
- El `refresh_token` se envía una vez por renovación (cada ~6 h) y se sustituye si el proveedor lo envía de nuevo.
- El `jti` viaja en toda sesión emitida por el callback (con o sin `refresh_token`); es lo que ata la sesión al token del proveedor cuando lo hay.
