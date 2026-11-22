# Renovación deslizante de la sesión

## Qué es

Follow-up OIDC: la sesión local se renueva sola cuando está a punto de caducar. Si al JWT le quedan 2 h o menos de sus 8 h, el *gate* emite una cookie nueva con la misma identidad y vida completa. Así el operador no vuelve a la portada en mitad de una jornada.

- Issue: #39
- Change OpenSpec: `refresh-sesion-deslizante`
- Plan: `plans/PLAN-019.md`

## Ficheros implicados

- `backend/src/auth/session.rs`: `RENEWAL_WINDOW`, `should_renew`, `renew`.
- `backend/src/auth/middleware.rs`: el *gate* añade la `Set-Cookie` renovada a la respuesta.
- `backend/src/api/mod.rs` y `backend/src/test_support.rs`: tests de integración y ayudas.
- `openspec/specs/autenticacion/spec.md`: contrato actualizado.

## Cómo funciona

- Cada petición protegida verifica el JWT. Si es válido y está dentro de la ventana, se reemite con `iat`/`exp` nuevos.
- La cookie nueva conserva `sub` y `email` y los mismos atributos: `HttpOnly`, `SameSite=Strict`, `Path=/`, `Secure` según el redirect, `Max-Age=8h`.
- El código y el cuerpo de la respuesta no cambian: solo se añade `Set-Cookie`.
- El fallo al renovar no convierte la petición en `500`: se registra y se sigue con el token actual.
- Fuera de la ventana no se renueva. Un token caducado o con firma inválida sigue dando `401`.

## Variables y configuración

No hay variables nuevas. La ventana es una constante independiente del módulo: `RENEWAL_WINDOW` (2 h).

## Cómo habilitar o desplegar

No requiere despliegue específico. Se publica con el build normal del backend.

## Decisiones

- **Renovación deslizante en el *gate*** y no en el frontend: el cliente no tiene que saber cuándo caduca el JWT ni pedir una renovación explícita.
- **Sin estado**: no se guardan tokens del proveedor ni sesiones en base de datos. La sesión sigue siendo un JWT autocontenido.
- **Fallo de renovación tolerado**: si reemitir falla, la petición continúa con el token actual, que sigue siendo válido hasta su `exp`.

## Riesgos aceptados

- **Sin tope absoluto**: cada token caduca, pero la sesión puede renovarse indefinidamente mientras el operador esté activo. Un tope exigiría estado en servidor o un claim extra.
- **Carreras entre renovaciones**: varias peticiones simultáneas dentro de la ventana pueden emitir varias cookies; la última en llegar gana en el navegador.
- **Carrera logout ↔ renovación**: una petición protegida en vuelo puede devolver su `Set-Cookie` renovada después de que el navegador procese el borrado del logout y resucitar la sesión. El diseño sin estado no permite revocarla en servidor. Impacto acotado: el operador cree haber cerrado sesión y sigue dentro hasta que caduque el token.
- **SSE**: la renovación se emite al abrir el flujo; no se renueva durante el stream.

## Verificación

- `cargo test --lib` → 381+ tests en verde.
- `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` → sin errores.
- `just check-all` → verde, incluido frontend y build.

## Follow-ups

- Evaluar una época de logout o revocación server-side para cerrar la carrera logout ↔ renovación.
- Valorar un tope absoluto de sesión si el panel se expone fuera de loopback.
