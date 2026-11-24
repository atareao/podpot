# Proposal — logout-epoch

## Why

La renovación deslizante de la cookie de sesión (ya implementada en refresh-sesion-deslizante) introduce una carrera con el logout: una petición protegida en vuelo puede devolver su Set-Cookie renovada después de que el navegador procese el borrado del logout y resucitar la sesión. El diseño sin estado no permite revocarla en servidor. El operador cree haber cerrado sesión y sigue dentro hasta que caduque el token.

## What Changes

- Añadir un epoch (marcador de tiempo de logout) al JWT de la cookie de sesión.
- El epoch se almacena en el token (claim logout_epoch) y se compara en cada renovación.
- POST /api/v1/auth/logout SHALL establecer un logout_epoch futuro en la cookie de borrado, indicando que la renovación debe ser rechazada hasta esa época.
- El gate SHALL comparar el logout_epoch del token con el actual: si el token tiene un logout_epoch menor o igual al actual, la renovación SHALL ser rechazada (401 + cookie de borrado).
- La renovación NO SHALL cambiar el logout_epoch del token: si un token ya tiene logout_epoch establecido, la renovación conserva ese valor.
- Sin cambios en el contrato visible de la API: mismos códigos y cuerpos.

## Impact

- backend/src/auth/session.rs: añadir logout_epoch al JWT, should_renew y renew lo respetan.
- backend/src/auth/middleware.rs: el gate compara logout_epoch antes de renovar.
- backend/src/api/auth.rs: logout establece logout_epoch en la cookie de borrado.
- Tests de integración: logout seguido de refresh en vuelo no resucite la sesión.

## Riesgos aceptados

- La cookie de borrado del logout lleva un logout_epoch que el navegador puede ignorar (es solo una cookie). El mecanismo funciona porque el gate compara el logout_epoch del token con el de la cookie: si el token tiene un logout_epoch menor o igual al de la cookie, la renovación es rechazada.
- Carrera benigna entre logout y refresh: el gate compara logout_epoch del token con el de la cookie; el refresh que llegue después del logout será rechazado.
