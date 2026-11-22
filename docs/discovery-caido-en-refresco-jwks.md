# Descubrimiento caído durante el refresco del JWKS

## Qué es

Cubre con test un camino que ya estaba implementado en la caché de JWKS: cuando el refresco del `jwks_uri` necesita resolver primero el descubrimiento (`discovery_cached`) y **es el descubrimiento el que falla**, el error se propaga tal cual y la caché anterior del JWKS no se toca. Antes de este cambio, ese desenlace concreto —refresco del JWKS que falla *por el descubrimiento*, no por el `jwks_uri`— no tenía prueba propia; era el follow-up registrado en `docs/cache-jwks-oidc.md`.

No hay cambio de comportamiento: el refresco ya resolvía el descubrimiento antes de pedir el conjunto y ya propagaba el error antes de escribir la caché. El cambio añade la infraestructura de test para tumbarlo y la prueba que fija el desenlace. Issue #43; change OpenSpec `discovery-caido-en-refresco-jwks`; capability `autenticacion`, requisito «Caché del JWKS con frescura acotada». Sin cambio del contrato HTTP.

## Ficheros implicados

### Backend (`backend/`)

- `src/test_support.rs` — `ProveedorFalso` gana `caido_descubrimiento: Arc<AtomicBool>` y `tumbar_el_descubrimiento()`. El `Mock` del descubrimiento se sirve por `lector_del_descubrimiento`, que responde `200` con el documento o `500` si la bandera está puesta (el mismo patrón que `lector_del_jwks` con `tumbar_el_jwks`).
- `src/auth/oidc.rs` — test `un_descubrimiento_caido_al_refrescar_el_jwks_da_un_502_sin_envenenar_la_cache`, en `mod tests`. Sin cambios de producción.

## Cómo funciona

El test siembra la caché con `cachear_el_jwks` (JWKS y descubrimiento cacheados, contador del `jwks_uri` en 1), **vacía la caché del descubrimiento** para obligar al refresco a pedirlo, tumba el descubrimiento y rota la clave. Con un ID token firmado por la clave nueva —un `kid` que la caché no publica— el flujo entra en el refresco forzado, `refrescar_bajo_candado` llama a `discovery_cached`, que falla, y el error se propaga **antes** de escribir la caché.

Se comprueba:

- La respuesta es `502` con la envoltura existente `{error:{code,message}}`, `code == "bad_gateway"` y el motivo genérico de la casa; el cuerpo **no** expone la causa subyacente.
- `peticiones_de_descubrimiento() == 2`: el refresco **sí** fue a pedir el documento caído, así que el test no pasa por vacío.
- La caché anterior del JWKS no se envenena: un token firmado con la clave **original** sigue verificando con el contador del `jwks_uri` intacto en 1, sin nueva lectura.

## Decisiones

- **La bandera del descubrimiento vive en el doble, como la del `jwks_uri`.** Así el escenario se activa desde la prueba y el camino feliz no paga nada: por defecto el descubrimiento sigue sano.
- **Se vacía la caché del descubrimiento a mano.** Es el único modo de que el refresco llegue a pedir el documento: con el descubrimiento ya cacheado —lo normal tras un login— el endpoint caído no se consulta y el escenario no se reproduciría.
- **El test es caracterización, no cambio de conducta.** Nace verde porque el camino ya era correcto; su valor es fijar el desenlace y evitar una regresión silenciosa (por ejemplo, cachear un conjunto vacío ante el fallo).

## Variables y configuración

Ninguna nueva.

## Cómo habilitar o desplegar

Sin despliegue específico: es un cambio de test. Entra con el build normal del backend.

## Follow-ups

- La carrera «un callbacks refresca mientras otro lee» en el descubrimiento no está cubierta; el descubrimiento no tiene el dedupe por marca del JWKS porque, a diferencia de este, no caduca.
- (Heredados de `docs/cache-jwks-oidc.md`) autorización por rol; logout con cookie inválida; refresh de tokens; chunk de antd >500 kB.
