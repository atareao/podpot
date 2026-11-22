# Proposal — refresh-sesion-deslizante

## Why

La sesión es un JWT local de 8 h en cookie `HttpOnly`. Hoy no se renueva: al caducar, el operador vuelve a la portada y tiene que entrar de nuevo aunque haya estado activo. El issue #39 pide renovación deslizante sin cambiar el contrato visible.

## What Changes

- El *gate* renueva la cookie cuando a un token válido le quedan la ventana de renovación o menos (2 h de las 8).
- La sesión renovada conserva `sub` y `email`, se firma de nuevo con `PODPOT_SECRET` y recibe `iat`/`exp` nuevos.
- La cookie renovada mantiene los mismos atributos: `HttpOnly`, `SameSite=Strict`, `Path=/`, `Secure` según el esquema del redirect y `Max-Age=8h`.
- Sin cambios de contrato: mismos cuerpos y códigos. Solo se añade `Set-Cookie` cuando toca renovar.
- Sin cambios de frontend. Sin estado en el servidor: no se guardan tokens del proveedor.

## Impact

- `backend/src/auth/session.rs`: ventana, `should_renew` y `renew`.
- `backend/src/auth/middleware.rs`: el *gate* añade la cookie renovada a la respuesta.
- No afecta a rutas públicas, logout, SSE ni modo `disabled`.

## Riesgos aceptados

- **Sin tope absoluto**: cada token caduca, pero la sesión puede renovarse indefinidamente mientras el operador esté activo. Un tope exigiría estado en servidor o un claim extra.
- **Carreras entre renovaciones**: varias peticiones simultáneas dentro de la ventana pueden emitir varias cookies; la última en llegar gana en el navegador.
- **Carrera logout ↔ renovación**: una petición protegida en vuelo puede devolver su `Set-Cookie` renovada después de que el navegador procese el borrado del logout y resucitar la sesión. El diseño sin estado no permite revocarla en servidor. Mitigación futura: época de logout o revocación server-side. Impacto acotado: el operador cree haber cerrado sesión y sigue dentro hasta que caduque el token.
- **SSE**: la renovación se emite al abrir el flujo; no se renueva durante el stream.
