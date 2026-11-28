# Spec Delta

## ADDED Requirements

### Requirement: Cabecera y tema del panel

El panel SHALL mostrar una cabecera fija con la marca «podpot», la identidad del usuario autenticado y un control de salida, sin barra lateral. SHALL ofrecer tema claro y oscuro mediante `ConfigProvider` de Ant Design, persistir la elección en `localStorage` y respetar `prefers-color-scheme` cuando no haya elección previa.

#### Scenario: Cabecera con identidad y salida

- **WHEN** una sesión autenticada abre el panel
- **THEN** SHALL mostrarse la marca, el nombre de usuario obtenido de la sesión y un control para cerrar sesión, sin sidebar

#### Scenario: Tema persistido

- **WHEN** el usuario cambia entre tema claro y oscuro y recarga la página
- **THEN** el panel SHALL conservar el tema elegido

#### Scenario: Preferencia del sistema

- **WHEN** no hay tema guardado y el sistema prefiere esquema oscuro
- **THEN** el panel SHALL usar el tema oscuro

### Requirement: Métricas resumidas del panel

El panel SHALL mostrar cuatro tarjetas de métricas calculadas a partir de la instantánea de unidades: total, en ejecución, paradas y fallidas. El panel NO SHALL mostrar una métrica de actualizaciones porque el backend no expone ese dato.

#### Scenario: Conteo desde la instantánea

- **WHEN** llega una instantánea con unidades en estados activo, inactivo y fallido
- **THEN** cada tarjeta SHALL reflejar el recuento correspondiente y actualizarse con el siguiente evento SSE

#### Scenario: Sin unidades

- **WHEN** la instantánea está vacía
- **THEN** las cuatro tarjetas SHALL mostrar cero

### Requirement: Búsqueda con debounce

El panel SHALL ofrecer un campo de búsqueda que filtre por `file_name`, `name`, `stack` y `kind` con un retardo aproximado de 250 ms, combinable con los filtros existentes por `kind` y `active_state`.

#### Scenario: Búsqueda por identidad

- **WHEN** el usuario escribe un fragmento del `file_name`
- **THEN** tras el retardo SHALL mostrarse solo las unidades coincidentes

#### Scenario: Combinación con filtros

- **WHEN** hay un filtro de estado activo y se escribe texto de búsqueda
- **THEN** SHALL aplicarse ambos criterios a la vez

#### Scenario: Sin resultados

- **WHEN** ninguna unidad coincide
- **THEN** SHALL mostrarse un estado vacío explícito, sin grupos vacíos ni errores

### Requirement: Modal de inspección de quadlet

El panel SHALL permitir abrir un modal de inspección con los campos devueltos por `GET /quadlets/{name}`: imagen, `container_name`, puertos publicados, volúmenes, entornos, red/subred y pod. El modal NO SHALL presentar campos no disponibles en la API.

#### Scenario: Inspección de una unidad

- **WHEN** el usuario activa la acción de inspección de una unidad
- **THEN** SHALL abrirse un modal con los campos reales de su quadlet

#### Scenario: Carga fallida

- **WHEN** la consulta de detalle falla
- **THEN** el modal SHALL mostrar el error y permitir cerrarse sin romper el panel

### Requirement: Diseño adaptable del panel

El panel SHALL adaptar su densidad por debajo de 768 px: las tarjetas reducen padding y etiquetas, y los controles conservan nombre accesible y navegación por teclado.

#### Scenario: Vista móvil

- **WHEN** el ancho del viewport es inferior a 768 px
- **THEN** las tarjetas SHALL mostrarse en una columna con padding reducido y sin perder acciones accesibles

#### Scenario: Vista de escritorio

- **WHEN** el ancho del viewport es igual o superior a 768 px
- **THEN** SHALL conservarse la agrupación en tarjetas con la densidad normal

### Requirement: Notificaciones y errores globales

El panel SHALL mostrar notificaciones temporales de aproximadamente 3 s de éxito, error o aviso tras cada acción, y SHALL envolver la aplicación en un `ErrorBoundary` que evite una pantalla en blanco ante errores de render.

#### Scenario: Notificación de éxito

- **WHEN** una acción sobre una unidad termina correctamente
- **THEN** SHALL mostrarse una notificación de éxito que desaparece sola

#### Scenario: Notificación de error

- **WHEN** una acción falla
- **THEN** SHALL mostrarse una notificación de error con el mensaje devuelto por la API

#### Scenario: Error de render

- **WHEN** un componente lanza un error durante el renderizado
- **THEN** el `ErrorBoundary` SHALL mostrar un mensaje de recuperación en lugar de una pantalla vacía

## MODIFIED Requirements

### Requirement: Listado agrupado por stack

Además de la identidad y el estado definidos en «Listado de quadlets», el panel SHALL mostrar tarjetas agrupadas por cada `stack` distinto, con una cabecera que identifique el stack y permita expandir o colapsar el grupo mediante ratón y teclado (`aria-expanded`); dentro, una tarjeta por unidad con su estado y acciones. Los grupos SHALL ordenarse por nombre de stack de forma case-insensitive; las unidades dentro de un grupo SHALL ordenarse de forma determinista (principales antes que auxiliares, luego por nombre). La vista sin filtros SHALL mostrar todas las unidades. La cabecera del grupo SHALL NO sustituir la identidad `file_name` ni las acciones de cada tarjeta.

#### Scenario: Unidades del mismo stack juntas

- **GIVEN** tres unidades con `stack == "foo"` y una con `stack == "bar"`
- **WHEN** se renderiza el listado
- **THEN** SHALL aparecer dos grupos, `bar` antes que `foo`, y las tres unidades de `foo` dentro del mismo grupo

#### Scenario: Estado visible por unidad

- **GIVEN** un grupo con una unidad activa y otra parada
- **WHEN** se renderiza el grupo
- **THEN** cada tarjeta SHALL mostrar su propio estado y sus acciones sin que la cabecera del grupo sustituya esa información

#### Scenario: Actualización en vivo sin reordenar

- **GIVEN** el listado agrupado visible
- **WHEN** llega un cambio de estado de una unidad
- **THEN** la unidad SHALL permanecer en su grupo y posición determinista, actualizándose solo su estado

#### Scenario: Expansión y colapso por teclado

- **GIVEN** un grupo visible con unidades
- **WHEN** el usuario activa el encabezado del grupo con ratón o teclado
- **THEN** las tarjetas SHALL ocultarse o mostrarse y el encabezado SHALL actualizar `aria-expanded`

### Requirement: Visor de logs de una quadlet

El panel SHALL permitir abrir un visor de logs para una unidad desde el listado en un modal; el visor SHALL ofrecer búsqueda de líneas y autodesplazamiento condicional. Al abrirlo SHALL suscribirse al canal SSE de logs de esa unidad y mostrar las líneas recibidas, distinguiendo su origen cuando proceda. El visor SHALL mantenerse al día con las líneas nuevas mientras esté abierto. Al cerrarlo SHALL darse de baja del canal. Si el canal no está disponible, el visor SHALL comunicar el error sin romper el resto del panel.

#### Scenario: Abrir el visor

- **WHEN** el usuario abre los logs de una unidad
- **THEN** SHALL mostrarse en un modal las líneas recibidas por el canal de esa unidad

#### Scenario: Líneas nuevas en vivo

- **WHEN** llega una línea nueva mientras el visor está abierto
- **THEN** SHALL añadirse a lo mostrado

#### Scenario: Cerrar el visor

- **WHEN** el usuario cierra el visor
- **THEN** SHALL darse de baja del canal

#### Scenario: Canal no disponible

- **WHEN** el canal de logs no puede establecerse o falla
- **THEN** el visor SHALL mostrar el error sin afectar al resto del panel

#### Scenario: Búsqueda de líneas

- **WHEN** el usuario escribe un término en el buscador del visor
- **THEN** SHALL mostrarse solo las líneas que lo contienen

### Requirement: Progreso de las acciones en el panel

El panel SHALL suscribirse al canal SSE de trabajos (`GET /api/v1/jobs/stream`) y mantener, por unidad, el conjunto de jobs en curso (alta al recibir la fase `new`, baja al recibir la fase `done`). SHALL mostrar por unidad el indicador «en curso» con el tipo de acción mientras haya un job para esa unidad, mediante un `Spin` y deshabilitando los botones de acción de esa unidad. Tras un cambio de estado informado por SSE, la unidad SHALL resaltarse durante 2 s. La suscripción SHALL cerrarse al desmontar el panel y SHALL reconectar automáticamente si la conexión se corta. Si el canal no está disponible, el panel SHALL seguir funcionando con su comportamiento actual, sin el indicador de progreso.

#### Scenario: Indicador al crearse un job

- **WHEN** llega un evento de fase `new` para una unidad
- **THEN** el panel SHALL mostrar esa unidad como «en curso» con el tipo de acción, con `Spin` y los botones deshabilitados

#### Scenario: Indicador al terminar el job

- **WHEN** llega un evento de fase `done` para una unidad
- **THEN** el panel SHALL dejar de mostrar el indicador de progreso y SHALL volver a habilitar los botones de esa unidad

#### Scenario: Baja al desmontar

- **WHEN** el panel se desmonta
- **THEN** SHALL cerrar la suscripción al canal de trabajos

#### Scenario: Canal no disponible

- **WHEN** el canal de trabajos no puede establecerse o se corta
- **THEN** el panel SHALL seguir funcionando sin el indicador de progreso y SHALL reintentar la suscripción

#### Scenario: Resaltado por cambio SSE

- **WHEN** una unidad cambia por un evento SSE
- **THEN** su tarjeta SHALL resaltarse durante 2 s
