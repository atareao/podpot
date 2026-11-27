# Tasks

> Checklist retrospectivo: los escenarios ya están implementados y verificados; todas las casillas se marcan completadas.

## Requirement: Sesión en cookie

- [x] Verificar que `POST /api/v1/auth/logout` elimina la cookie de sesión y establece un `logout_epoch` futuro en la cookie de borrado.
- [x] Verificar que una ruta de negocio con un token válido cuyo `logout_epoch` es menor o igual al actual devuelve `401` con la envoltura y no envía `Set-Cookie` de renovación.
- [x] Verificar que la renovación de un token con `logout_epoch` establecido devuelve `200` con la misma respuesta y una `Set-Cookie` de sesión nueva que conserva el mismo `logout_epoch`.
- [x] Verificar que la renovación de un token sin `logout_epoch` devuelve `200` con la misma respuesta y una `Set-Cookie` de sesión nueva sin `logout_epoch`.
