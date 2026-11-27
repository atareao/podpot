## MODIFIED Requirements

### Requirement: Resumen de unidad con stack

El resumen de cada unidad SHALL incluir un campo `stack: String` derivado de forma determinista de `file_name` y `kind`. Para unidades principales (`container`, `pod`, `kube`, `build`, `image`) el stack SHALL ser el nombre base del fichero sin extensión. Para unidades auxiliares (`volume`, `network`) el stack SHALL ser el nombre base sin el sufijo `-volume`/`-network` cuando éste esté presente y no vacíe el nombre; en caso contrario SHALL ser el nombre base completo. Si no hay `file_name` o el tipo no es reconocido, el stack SHALL derivarse del nombre de la unidad sin `.service`. El campo SHALL ser estable ante cambios de `active_state` o `sub_state`.

**Nueva regla (prefijo + override):**
- **Regla base**: el stack SHALL ser el primer segmento del stem antes del primer `-` (case-insensitive para la decisión, conservando la grafía original del primer segmento). Si no hay `-`, el stack SHALL ser el stem completo.
- **Override explícito**: un archivo `groups.toml` en el directorio de la fuente del quadlet SHALL permitir sobrescribir el stack mediante una tabla `[mappings]` con claves `file_name` o `unit_name`. El override SHALL tener prioridad sobre la regla base.
- **Fallbacks**: si no hay `file_name`, se SHALL usar `unit_name` sin `.service` y sin sufijo de tipo. El campo `stack` SHALL ser estable y no SHALL depender de `active_state` ni `sub_state`.

#### Scenario: Principal y auxiliares comparten stack

- **GIVEN** los ficheros `foo.container`, `foo.volume` y `foo.network`
- **WHEN** se lista `GET /api/v1/units`
- **THEN** `foo.service`, `foo-volume.service` y `foo-network.service` SHALL tener `stack == "foo"`

#### Scenario: Recurso auxiliar independiente

- **GIVEN** el fichero `backup.volume` sin un `backup.container` asociado
- **WHEN** se lista el estado
- **THEN** `backup-volume.service` SHALL tener `stack == "backup"` y formar su propio grupo

#### Scenario: Sufijo en la unidad principal no se recorta

- **GIVEN** el fichero `foo-volume.container`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `foo` (prefijo antes del primer `-`)

#### Scenario: Sin tipo reconocido

- **GIVEN** una unidad sin `file_name` o con `kind == null`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL derivarse del nombre de la unidad sin el sufijo `.service`, sin panic ni valor vacío

#### Scenario: Estabilidad en vivo

- **GIVEN** una unidad con `stack == "foo"`
- **WHEN** cambia su `active_state` por un evento en vivo
- **THEN** su `stack` SHALL permanecer `foo`

#### Scenario: Regla base con prefijo

- **GIVEN** una unidad con `file_name = "atareao-mariadb.container"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"atareao"`

#### Scenario: Regla base sin prefijo

- **GIVEN** una unidad con `file_name = "standalone.container"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"standalone"`

#### Scenario: Fallback a unit_name sin file_name

- **GIVEN** una unidad sin `file_name` pero con `unit_name = "centinela-app.service"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"centinela"`

#### Scenario: Fallback a unit_name sin sufijo

- **GIVEN** una unidad sin `file_name` pero con `unit_name = "singleton.service"`
- **WHEN** se calcula el stack
- **THEN** el stack SHALL ser `"singleton"`

#### Scenario: Override explícito en groups.toml

- **GIVEN** `groups.toml` contiene `"atareao-mariadb.container" = "db-cluster"`
- **WHEN** se calcula el stack para `atareao-mariadb.container`
- **THEN** el stack SHALL ser `"db-cluster"` (override gana sobre regla base)

#### Scenario: Override explícito para unit_name

- **GIVEN** `groups.toml` contiene `"centinela-app.service" = "monitoring"`
- **WHEN** se calcula el stack para `centinela-app.service`
- **THEN** el stack SHALL ser `"monitoring"` (override gana sobre regla base)
