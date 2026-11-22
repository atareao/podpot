# Spec Delta — ui

## Purpose

El panel web (React + Ant Design) que muestra el estado de las quadlets y permite controlarlas. Este delta añade el reparto del bundle de producción en chunks.

## ADDED Requirements

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
