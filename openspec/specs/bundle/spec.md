# bundle Specification

## Purpose
TBD - created by archiving change antd-bundle-budget. Update Purpose after archive.

## Requirements

### Requirement: Presupuesto individual por chunk de Ant Design
Todo chunk de producción cuyo nombre empiece por `vendor-antd` SHALL tener un tamaño raw menor que `500 * 1024` bytes. El presupuesto SHALL aplicarse por chunk individual; la suma de chunks NO SHALL utilizarse como criterio de este requisito.

#### Scenario: Build de producción dentro del presupuesto
- **GIVEN** una configuración de Vite en modo `production` con `manualChunks` que separa `antd` y sus dependencias `rc-*`
- **WHEN** se ejecuta un build con `write: false`
- **THEN** SHALL existir al menos un chunk `vendor-antd*`
- **AND** cada chunk `vendor-antd*` SHALL tener `code.length < 500 * 1024`

#### Scenario: Chunk que excede el presupuesto
- **GIVEN** un build de producción en el que un chunk `vendor-antd*` pesa `>= 500 * 1024` bytes
- **WHEN** se ejecuta el test de presupuesto
- **THEN** el test SHALL fallar con un mensaje que incluya el nombre del chunk y su tamaño en kB

### Requirement: Ant Design permanece en la aplicación
La optimización SHALL conservar la dependencia `antd`, `@ant-design/icons` y los componentes usados por el panel. NO SHALL eliminarse ni sustituirse por otra librería.

#### Scenario: Dependencia presente tras la optimización
- **GIVEN** el build de producción resultante
- **WHEN** se inspeccionan `package.json` y el código de la aplicación
- **THEN** `antd` SHALL seguir declarado como dependencia
- **AND** los componentes SHALL seguir importándose desde `antd`

### Requirement: Test de presupuesto tipado
El test de presupuesto SHALL compilar sin errores de TypeScript bajo la configuración estricta del proyecto.

#### Scenario: Salida de build tipada
- **GIVEN** `build()` de Vite devuelve `RollupOutput | RollupOutput[]`
- **WHEN** TypeScript comprueba `frontend/tests/tamanoDelBundle.test.ts`
- **THEN** NO SHALL haber errores `TS2339` ni `TS7006`
- **AND** el filtrado de chunks SHALL usar el discriminante `type === 'chunk'`

#### Scenario: Aislamiento del test
- **GIVEN** un entorno de ejecución Node y `testTimeout: 60000`
- **WHEN** se ejecuta `npx vitest run tests/tamanoDelBundle.test.ts`
- **THEN** el test SHALL ejecutarse sin escribir ficheros en disco (`write: false`)
