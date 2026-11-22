# PLAN-019 — Refresh deslizante de la sesión

- Issue: #39
- Change OpenSpec: `refresh-sesion-deslizante`
- Rama: `feature/refresh-sesion`
- Base: `development`

## Objetivo

Renovar la cookie de sesión de forma deslizante cuando al JWT le quede menos de 2 horas de las 8 de vida, sin cambiar el contrato visible ni guardar tokens del proveedor.

## Decisiones

- Ventana de renovación: 2 horas (`SESSION_LIFETIME / 4`), constante del módulo.
- La sesión renovada conserva `sub` y `email`; se firma de nuevo con `iat`/`exp` nuevos.
- La cookie renovada replica los atributos del login (`HttpOnly`, `SameSite=Strict`, `Path=/`, `Secure` según el redirect, `Max-Age=8h`).
- El *gate* añade `Set-Cookie` a la respuesta; no toca el cuerpo ni el código.
- Si la renovación falla al firmar, se registra y la petición continúa con la sesión actual (no se convierte un fallo interno en un `500` evitable).
- Sin tope absoluto de sesión: se acepta el riesgo y se anota como follow-up.

## Tareas TDD

1. **RED**
   - [ ] Tests unitarios de `session.rs`: ventana, identidad conservada, `exp` alargado.
   - [ ] Tests de integración del *gate*: renueva dentro de la ventana; no renueva fuera; `401` sin cookie nueva con sesión inválida.
   - [ ] Confirmar fallos por comportamiento, no por compilación.
2. **GREEN**
   - [ ] Ventana y emisión con identidad en `session.rs`.
   - [ ] *Gate* añade la cookie renovada a la respuesta.
   - [ ] Tests focalizados en verde.
3. **REFACTOR**
   - [ ] Comentarios y documentación del módulo.
   - [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`, `just check-all`.
4. **Cierre**
   - [ ] `docs/refresh-sesion-deslizante.md`.
   - [ ] `HANDOFF.md`, commit gitmoji, push y PR a `development`.
   - [ ] Archivar change y plan; cerrar issue #39.

## Riesgos

- Sin tope absoluto: sesión indefinida con uso continuo. Follow-up: claim adicional con la emisión original.
- Varias peticiones simultáneas en la ventana emiten varias cookies válidas; gana la última.
- SSE: la renovación se emite al abrir el flujo; no se renueva durante el stream.

## Verificación final

- [ ] `just check-all` verde.
- [ ] `openspec validate refresh-sesion-deslizante --strict` en verde.
- [ ] PR #<pendiente> mergeado con CI verde.
