# Logout epoch — revocación server-side

## Qué es

Seguimiento de `refresh-sesion-deslizante` (issue #58): cierra la carrera *logout ↔ renovación* que dejaba abierta la renovación deslizante. Antes, si el usuario cerraba sesión y había una petición en vuelo pidiendo una renovación automática, esta podía resucitar la sesión incluso después del cierre. El operador veía el logout como efectivo pero seguía dentro hasta que caducaba el token.

La solución usa dos mecanismos complementarios:

1. **Mapa de revocados por logout** (`AuthState::cerrar_sesion_por_logout`) — mecanismo principal en el *gate*: al cerrar sesión, se marca el `jti` como revocado durante [`SESSION_LIFETIME`] y el gate lo detecta antes de intentar renovar.
2. **Claim `logout_epoch` en el JWT** — defensa en profundidad: se añade un campo `logout_epoch: u64` a los claims; si es `> 0` y aún no ha pasado, `should_renew` devuelve `false` aunque esté dentro de la ventana.

- Issue: #58
- Change OpenSpec: `logout-epoch`
- Plan: `plans/PLAN-026.md`
- PR: #59

## Ficheros implicados

- `backend/src/auth/session.rs`: claim `logout_epoch` en `Claims`, lógica en `should_renew` y `renew`.
- `backend/src/auth/state.rs`: métodos `cerrar_sesion_por_logout` y `esta_revocado_por_logout`; mapa de revocados con purga.
- `backend/src/api/auth.rs`: handler `logout` extrae `jti` y llama a `cerrar_sesion_por_logout`.
- `backend/src/auth/middleware.rs`: el *gate* verifica `esta_revocado_por_logout` antes de llamar a `should_renew`.
- `openspec/specs/autenticacion/spec.md`: contrato actualizado con nuevos escenarios BDD.

## Cómo funciona

El flujo completo de un logout bloqueante es:

```
Logout handler         Gate (con_cookie)            should_renew
───────────────        ───────────────────          ──────────────
extrae jti de cookie   ← verify JWT → claims OK    claims.exp > ahora
                        if esta_revocado(jti)       AND
                          → Revocada (OIDC)         claims.exp <= ahora + ventana
                        
                        if esta_revocado_por_logout(jti)
                          → Revocada ← [NUEVO]      logout_epoch bloquea
                          
                        else should_renew(...)      
                          → Valida (+ Set-Cookie)
```

1. **Usuario pide `/auth/logout`**: se extrae el `jti` de la cookie (si está presente), se marca en el mapa de revocados con `bloqueo_hasta = Instant::now() + SESSION_LIFETIME`, y se emite la cookie de borrado.
2. **Petición protegida posterior con la misma cookie**: el gate verifica primero revocación OIDC, luego llama a `esta_revocado_por_logout(jti)`. Si encuentra el marcaje vigente, responde `401` + borrado sin intentar renovar.
3. **Fallback JWT**: si por cualquier vía el gate omite esta verificación, `should_renew` comprueba también `claims.logout_epoch >= now` para rechazar.
4. **Renovación conserva `logout_epoch`**: si el token original tenía `logout_epoch > 0`, la nueva cookie lo mantiene intacto para que las siguientes peticiones sigan detectando el cierre.

## Variables y configuración

No hay variables nuevas. La duración del bloqueo usa `SESSION_LIFETIME` (constante del módulo).

## Decisiones

- **Doble capa**: mapa de revocados en *gate* (rápido, O(1)) + check JWT en `should_renew` (defensa en profundidad). Esto cubre ambos caminos posibles de renegociación.
- **Bloqueo basado en `Instant` monótono**, no Unix timestamp: evita problemas de reloj del sistema y comparaciones cruzadas. Se compara con `saturating_duration_since` para determinar expiración.
- **Purga al insertar**: igual que las otras entradas del mapa (`revocar_jti`, `logins`), se eliminan las entradas viejas justo antes de insertar la nueva, evitando crecimiento indefinido.
- **Contrato inmutable**: no cambia ningún código ni cuerpo de respuesta visible. Solo se añaden `Set-Cookie` de borrado cuando corresponde.

## Riesgos aceptados

- **Memoria del mapa**: crece solo con JTIs activos (uno por sesión logueada) y se purga al insertar cada nuevo entrada. El mapa tiene un tamaño máximo equivalente a las sesiones concurrentes en el momento pico.
- **Race entre logout y refresh**: el gate lee el mapa bajo lock y evalúa `esta_revocado_por_logout` antes de `should_renew`. No hay deadlock porque cada función obtiene su propio lock brevemente.
- **Exposición fuera de loopback**: si el panel se hace público más allá de localhost, considerar limitar `SESSION_LIFETIME` o añadir validación de IP.

## Verificación

- `cargo test --lib` → 412 tests en verde (incluye 10 nuevos).
- `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` → sin errores.
- `just check-all` → verde, incluido frontend y build.

## Follow-ups

- Evaluar si el valor de `SESSION_LIFETIME` (día completo) es adecuado o debería ser ajustable por configuración del administrador.
- Considerar exposición de métricas Prometheus del mapa de revocados para monitorizar actividad.
- Cerrar el follow-up de `refresh-tokens-proveedor` sobre el JTI compartido con la revocación OIDC si se considera necesaria una separación más estricta.
