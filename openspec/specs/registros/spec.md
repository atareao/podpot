# registros Specification

## Purpose
El canal que lleva al panel los logs del contenedor de una quadlet, leídos de la API de Podman por su socket y servidos por SSE.

## Requirements

### Requirement: Canal SSE de logs de una quadlet

El servidor SHALL exponer `GET /api/v1/units/{name}/logs/stream` como un flujo `text/event-stream`. Al establecer la conexión SHALL enviar primero las últimas líneas del contenedor (cola configurable) y después, mientras la conexión siga abierta, cada nueva línea según se produzca. Cada línea SHALL emitirse como un evento `log` cuyo contenido SHALL indicar la línea y su origen (salida estándar o error estándar). El flujo SHALL indicar un `retry` y SHALL enviar latidos; SHALL emitir un evento terminal cuando el stream de logs termine; y SHALL cerrarse cuando el servidor se apague.

#### Scenario: Cola inicial al conectar

- **WHEN** un cliente abre el canal de logs de una quadlet con contenedor
- **THEN** SHALL recibir primero las últimas líneas del contenedor sin esperar a que se produzcan otras nuevas

#### Scenario: Líneas nuevas según se producen

- **WHEN** el contenedor escribe una nueva línea estando la conexión abierta
- **THEN** SHALL emitirse un evento `log` con esa línea y su origen

#### Scenario: Fin del stream

- **WHEN** el contenedor deja de producir y el stream de logs termina
- **THEN** SHALL emitirse un evento terminal y el flujo SHALL finalizar de forma ordenada

#### Scenario: Cierre con el servidor

- **WHEN** el servidor se apaga con un visor de logs abierto
- **THEN** el flujo SHALL cerrarse sin bloquear el apagado ordenado

### Requirement: Fuente de logs por la API de Podman

Los logs SHALL leerse de la API de Podman a través de su socket, sin invocar binarios externos. El contenedor de la quadlet SHALL localizarse por la etiqueta que systemd pone a los contenedores que gestiona, no por el nombre del fichero. Si no hay contenedor para la unidad, la petición SHALL fallar de forma explícita sin abrir el flujo. La lectura SHALL seguir el stream cuando el contenedor aún produce y SHALL terminar cuando la unidad no sigue produciendo.

#### Scenario: Contenedor localizado por etiqueta

- **WHEN** se piden los logs de una quadlet cuyo contenedor lleva la etiqueta de systemd
- **THEN** SHALL leerse el stream de ese contenedor

#### Scenario: Unidad sin contenedor

- **WHEN** se piden los logs de una unidad que no tiene contenedor (p. ej. un volumen o una red)
- **THEN** la petición SHALL fallar con un error explícito y NO SHALL abrir el flujo de eventos

#### Scenario: Contenedor sin TTY

- **WHEN** el contenedor no usa TTY
- **THEN** el stream SHALL demultiplexarse en líneas, cada una con su origen

#### Scenario: Contenedor con TTY

- **WHEN** el contenedor usa TTY
- **THEN** el stream SHALL leerse como líneas de texto sin marcos

### Requirement: Puerto de registros con doble de pruebas

La capa SHALL exponer un puerto de registros que entregue líneas del contenedor, de modo que el canal SSE no dependa de la API de Podman. El doble de pruebas SHALL poder entregar líneas sin socket real.

#### Scenario: El doble entrega líneas

- **WHEN** una prueba usa el doble de registros y este entrega líneas
- **THEN** el canal SSE SHALL emitirlas como eventos `log`, sin necesidad de un socket de Podman real
