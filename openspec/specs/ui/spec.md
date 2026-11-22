# ui Specification

## Purpose
El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas.

## Requirements

### Requirement: Listado de quadlets

El panel SHALL mostrar un listado con cada unidad quadlet y su estado actual, obtenido de la API. Cada quadlet SHALL llevar su identidad real —el `file_name` publicado por la API— que es la clave con la que se la opera. La tabla SHALL mostrar el `file_name` como identidad primaria de cada fila, con el nombre de unidad systemd (`name`) como dato secundario visible, y SHALL usar el `file_name` en los nombres accesibles de las acciones y del progreso. La comparación interna del progreso y de la acción en curso SHALL seguir usando el nombre de unidad, porque es el identificador que publican el canal de trabajos y la API de acciones.

#### Scenario: Vista de estado

- **WHEN** el usuario abre el panel
- **THEN** SHALL ver cada quadlet con su estado (en ejecución, parada, fallida, …)

#### Scenario: Identidad publicada por la API

- **WHEN** el panel recibe el listado de `GET /api/v1/units`
- **THEN** SHALL tomar el `file_name` de cada quadlet como su identidad, sin reconstruirlo a partir del nombre de unidad

#### Scenario: Identidad primaria de la fila

- **WHEN** el panel muestra el listado de quadlets
- **THEN** cada fila SHALL mostrar el `file_name` como identidad primaria
- **AND** SHALL mostrar el nombre de unidad como dato secundario

#### Scenario: Nombres accesibles coherentes con la identidad

- **WHEN** una fila ofrece acciones o muestra progreso
- **THEN** los nombres accesibles de las acciones y del progreso SHALL usar el `file_name` de la quadlet

#### Scenario: Estado de acción en curso por nombre de unidad

- **WHEN** una acción está en curso sobre una unidad
- **THEN** la tabla SHALL casar el indicador de progreso y el bloqueo de acciones por el nombre de unidad, que es el identificador que publican el canal de trabajos y la API de acciones

---

### Requirement: Control desde el panel

El panel SHALL permitir arrancar, parar y reiniciar una unidad y SHALL reflejar el resultado, refrescando el estado tras la acción. Mientras una acción está en curso SHALL mostrar su progreso por unidad, según el canal de trabajos (ver «Progreso de las acciones en el panel»).

#### Scenario: Arrancar desde el panel

- **WHEN** el usuario pulsa «arrancar» en una quadlet parada
- **THEN** el panel SHALL llamar a la API, mostrar el resultado y refrescar el estado de esa unidad

#### Scenario: Progreso mientras la acción está en curso

- **WHEN** una acción sobre una unidad tiene un job en curso
- **THEN** el panel SHALL mostrar en esa unidad un indicador de que la acción está en curso

#### Scenario: Error de acción

- **WHEN** la API responde con error a una acción
- **THEN** el panel SHALL mostrar un mensaje de error y mantener el estado real de la unidad

### Requirement: Creación desde el panel

El panel SHALL ofrecer un formulario de creación (Ant Design) con un selector de tipo de quadlet y los campos mínimos del tipo elegido. Al crear con éxito SHALL refrescar el listado de quadlets; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Creación correcta desde el panel

- **WHEN** el usuario rellena el formulario y confirma la creación
- **THEN** el panel SHALL llamar a la API y, al recibir éxito, refrescar el listado de quadlets

#### Scenario: Selector de tipo

- **WHEN** el usuario elige un tipo en el formulario
- **THEN** SHALL mostrarse los campos mínimos correspondientes a ese tipo

#### Scenario: Error de creación

- **WHEN** la API responde con error a la creación
- **THEN** el panel SHALL mostrar el `error.message` recibido sin refrescar el listado como si hubiera tenido éxito

### Requirement: Consulta y precarga al editar una quadlet

El panel SHALL leer la definición de una quadlet existente antes de editarla (vía `GET /api/v1/quadlets/{name}`) y SHALL precargar el formulario con su tipo y sus campos reales. Al fallar la lectura SHALL mostrar el `error.message` devuelto por la API y NO SHALL abrir el formulario con valores inventados.

#### Scenario: Precarga de campos

- **WHEN** se abre la edición de una quadlet existente
- **THEN** el formulario SHALL precargarse con el tipo y los campos devueltos por la API

#### Scenario: Error de consulta

- **WHEN** la lectura de la quadlet falla
- **THEN** el panel SHALL mostrar el `error.message` devuelto por la API

### Requirement: Edición de quadlets desde el panel

El panel SHALL ofrecer la acción de editar una quadlet existente, reutilizando el formulario de creación precargado con el tipo y los campos de la quadlet seleccionada. Al guardar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Edición correcta

- **WHEN** se guarda una edición válida desde el panel
- **THEN** la API SHALL responder con éxito y el listado SHALL refrescarse

#### Scenario: Error de edición

- **WHEN** la API rechaza una edición
- **THEN** el panel SHALL mostrar el `error.message` devuelto

### Requirement: Borrado de quadlets desde el panel

El panel SHALL ofrecer la acción de borrar una quadlet existente, solicitando confirmación antes de ejecutarla (acción destructiva). La confirmación y el aviso de éxito SHALL identificar la quadlet con la misma identidad que la fila y los nombres accesibles, es decir, el `file_name` publicado por la API. Al confirmar con éxito SHALL refrescar el listado; al fallar SHALL mostrar el `error.message` devuelto por la API.

#### Scenario: Confirmación antes de borrar

- **WHEN** se pulsa borrar sobre una quadlet
- **THEN** el panel SHALL pedir confirmación antes de enviar la petición

#### Scenario: Confirmación con la identidad publicada

- **WHEN** el panel muestra la confirmación de borrado de una quadlet
- **THEN** el título y la descripción SHALL identificar la quadlet por su `file_name`
- **AND** NO SHALL usar el nombre de unidad como identidad principal cuando el `file_name` esté disponible

#### Scenario: Aviso de borrado con la identidad publicada

- **WHEN** el borrado se completa con éxito
- **THEN** el aviso SHALL identificar la quadlet por su `file_name`
- **AND** NO SHALL usar el nombre de unidad como identidad principal cuando el `file_name` esté disponible

#### Scenario: Borrado confirmado

- **WHEN** el usuario confirma el borrado y la API responde con éxito
- **THEN** el listado SHALL refrescarse sin la quadlet borrada

#### Scenario: Borrado cancelado

- **WHEN** el usuario cancela la confirmación
- **THEN** NO SHALL enviarse petición alguna de borrado

#### Scenario: Fallback defensivo ante identidad vacía

- **WHEN** una quadlet llega con `file_name` vacío
- **THEN** la confirmación y el aviso SHALL usar el nombre de unidad como identidad
- **AND** NO SHALL renderizar el literal `null`

### Requirement: Identidad de quadlet para las acciones

El panel SHALL usar el `file_name` que publica la API como identidad de la quadlet para las acciones de editar y borrar. El panel NO SHALL deducir el nombre de fichero invirtiendo el nombre de unidad, porque esa regla inversa no es inyectiva y puede apuntar al fichero equivocado. El contrato de la API SHALL tratar `file_name` como cadena no nula; la UI SHALL limitarse a un fallback defensivo ante cadena vacía.

#### Scenario: Editar y borrar usan la identidad publicada

- **WHEN** el usuario edita o borra una quadlet del listado
- **THEN** el panel SHALL operar sobre el `file_name` recibido de la API para esa quadlet

#### Scenario: Dos quadlets con nombres de unidad colisionados

- **WHEN** el listado contiene dos quadlets cuyos nombres de unidad no permiten distinguirlas (p. ej. `web-volume.service`)
- **THEN** el panel SHALL operar sobre el `file_name` correcto de cada una, sin confundirlas

#### Scenario: Fallback defensivo ante identidad vacía

- **WHEN** una quadlet llega con `file_name` vacío
- **THEN** la UI SHALL usar el nombre de unidad como identidad mostrada y accesible
- **AND** NO SHALL renderizar el literal `null`

### Requirement: Panel en vivo

El panel SHALL mantenerse al día con el estado real de las quadlets suscribiéndose al canal SSE (`GET /api/v1/units/stream`). Al recibir un evento SHALL actualizar el listado con la instantánea recibida, sin esperar a una acción del usuario. La suscripción SHALL cerrarse al desmontar el panel y SHALL reconectar automáticamente si la conexión se corta. Si el canal no está disponible, el panel SHALL seguir funcionando con la carga inicial de `GET /api/v1/units`.

#### Scenario: Cambio reflejado sin acción del usuario

- **WHEN** cambia el estado de una quadlet fuera del panel (otro cliente, `systemctl --user`, un fallo)
- **THEN** el panel SHALL reflejar el nuevo estado al recibir el evento del canal

#### Scenario: Baja al desmontar

- **WHEN** el panel se desmonta
- **THEN** SHALL cerrar la suscripción al canal

#### Scenario: Canal no disponible

- **WHEN** el canal SSE no puede establecerse o se corta
- **THEN** el panel SHALL seguir mostrando el listado de la carga inicial y SHALL reintentar la suscripción

### Requirement: Inicio de sesión en el panel

El panel SHALL consultar el modo de autenticación anunciado por la salud. Cuando el modo sea `oidc`, SHALL exigir sesión para acceder a la vista de unidades: sin sesión, SHALL mostrar una portada pública con un botón que lleve al inicio de sesión del servidor. Cuando el modo sea `disabled`, SHALL permitir el acceso sin sesión (dev). Al perder la sesión (respuesta `401`), el panel SHALL volver a la portada pública.

#### Scenario: Modo deshabilitado no exige sesión

- **WHEN** la salud anuncia modo `disabled`
- **THEN** el panel SHALL mostrar la vista de unidades sin exigir sesión

#### Scenario: Modo OIDC sin sesión

- **WHEN** la salud anuncia modo `oidc` y no hay sesión
- **THEN** el panel SHALL mostrar la portada pública con el botón de inicio de sesión, no la vista de unidades

#### Scenario: Botón de inicio de sesión

- **WHEN** el usuario pulsa el botón de inicio de sesión
- **THEN** SHALL navegar al inicio de sesión del servidor

#### Scenario: Sesión perdida durante el uso

- **WHEN** una petición del panel recibe `401`
- **THEN** el panel SHALL considerarse sin sesión y volver a la portada pública

### Requirement: Visor de logs de una quadlet

El panel SHALL permitir abrir un visor de logs para una unidad desde el listado. Al abrirlo SHALL suscribirse al canal SSE de logs de esa unidad y mostrar las líneas recibidas, distinguiendo su origen cuando proceda. El visor SHALL mantenerse al día con las líneas nuevas mientras esté abierto. Al cerrarlo SHALL darse de baja del canal. Si el canal no está disponible, el visor SHALL comunicar el error sin romper el resto del panel.

#### Scenario: Abrir el visor

- **WHEN** el usuario abre los logs de una unidad
- **THEN** SHALL mostrarse las líneas recibidas por el canal de esa unidad

#### Scenario: Líneas nuevas en vivo

- **WHEN** llega una línea nueva mientras el visor está abierto
- **THEN** SHALL añadirse a lo mostrado

#### Scenario: Cerrar el visor

- **WHEN** el usuario cierra el visor
- **THEN** SHALL darse de baja del canal

#### Scenario: Canal no disponible

- **WHEN** el canal de logs no puede establecerse o falla
- **THEN** el visor SHALL mostrar el error sin afectar al resto del panel

### Requirement: Progreso de las acciones en el panel

El panel SHALL suscribirse al canal SSE de trabajos (`GET /api/v1/jobs/stream`) y mantener, por unidad, el conjunto de jobs en curso (alta al recibir la fase `new`, baja al recibir la fase `done`). SHALL mostrar por unidad el indicador «en curso» con el tipo de acción mientras haya un job para esa unidad. La suscripción SHALL cerrarse al desmontar el panel y SHALL reconectar automáticamente si la conexión se corta. Si el canal no está disponible, el panel SHALL seguir funcionando con su comportamiento actual, sin el indicador de progreso.

#### Scenario: Indicador al crearse un job

- **WHEN** llega un evento de fase `new` para una unidad
- **THEN** el panel SHALL mostrar esa unidad como «en curso» con el tipo de acción

#### Scenario: Indicador al terminar el job

- **WHEN** llega un evento de fase `done` para una unidad
- **THEN** el panel SHALL dejar de mostrar el indicador de progreso de esa unidad

#### Scenario: Baja al desmontar

- **WHEN** el panel se desmonta
- **THEN** SHALL cerrar la suscripción al canal de trabajos

#### Scenario: Canal no disponible

- **WHEN** el canal de trabajos no puede establecerse o se corta
- **THEN** el panel SHALL seguir funcionando sin el indicador de progreso y SHALL reintentar la suscripción

### Requirement: Autodesplazamiento condicional del visor de registros

El visor de registros SHALL autodesplazarse al fondo al llegar líneas nuevas cuando el usuario esté **pegado al fondo** (a una distancia de 24 px o menos respecto del final del contenido). Si el usuario ha subido a leer (distancia mayor de 24 px), el visor NO SHALL desplazarle el scroll al llegar líneas nuevas. Al (re)abrir el visor SHALL partirse de la consideración de «pegado al fondo».

#### Scenario: Llegan líneas estando el usuario pegado al fondo

- **WHEN** llega una línea nueva y el usuario está a 24 px o menos del fondo
- **THEN** el visor SHALL desplazarse al fondo

#### Scenario: El usuario subió a leer

- **WHEN** llega una línea nueva y el usuario está a más de 24 px del fondo
- **THEN** el visor NO SHALL desplazarse

#### Scenario: Umbral delimitado por la distancia al fondo

- **WHEN** la distancia del usuario al fondo está en el límite de 24 px
- **THEN** el visor SHALL considerarse «pegado al fondo» y SHALL desplazarse

#### Scenario: Reapertura del visor

- **WHEN** se vuelve a abrir el visor de una unidad
- **THEN** volverá a considerarse «pegado al fondo» hasta que el usuario se desplace

### Requirement: Bundle del panel dividido en chunks por proveedor

El build de producción del panel SHALL dividir el bundle en varios chunks por dependencia/proveedor (al menos `antd`, `react`/`react-dom` y `react-query` separados del código de la aplicación), de manera que el chunk de entrada NO SHALL superar el umbral de aviso de 500 kB. Los chunks correspondientes a dependencias estables SHALL conservar su hash entre despliegues, de modo que un cambio en el código de la aplicación no SHALL invalidar la caché de las dependencias que no han cambiado. El comportamiento del panel NO SHALL cambiar: mismas rutas, mismas acciones y los mismos tests en verde.

#### Scenario: Build sin aviso de chunk grande

- **WHEN** se ejecuta el build de producción del panel
- **THEN** el chunk de entrada SHALL quedar por debajo de 500 kB
- **AND** NO SHALL emitirse el aviso de chunks mayores de 500 kB

#### Scenario: Dependencias cacheadas por separado

- **WHEN** cambia el código de la aplicación y se reconstruye el panel
- **THEN** los chunks de las dependencias que no han cambiado SHALL conservar su nombre (hash estable), de modo que la caché del navegador siga siendo válida para ellas

#### Scenario: Sin cambio de comportamiento

- **WHEN** se carga el panel tras el cambio
- **THEN** SHALL comportarse igual que antes (mismas rutas y acciones)
- **AND** los tests de frontend SHALL seguir en verde
