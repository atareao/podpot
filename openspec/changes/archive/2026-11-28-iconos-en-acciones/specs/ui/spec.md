# Spec Delta

## ADDED Requirements

### Requirement: Controles de acción con iconos

El panel SHALL presentar las acciones disponibles sobre una unidad (arrancar, parar, reiniciar, editar, ver logs y borrar) como iconos en lugar de etiquetas textuales visibles. Cada control SHALL exponer el nombre de su acción como nombre accesible (por ejemplo mediante `aria-label` o tooltip) y SHALL conservar exactamente el comportamiento previo de la acción al ser activado. El panel NO SHALL mostrar la palabra de la acción como texto visible dentro del control.

#### Scenario: Presentación de las acciones

- **WHEN** el panel renderiza la columna de acciones de una unidad
- **THEN** cada acción SHALL aparecer como un icono sin texto visible de la acción y SHALL exponer su nombre accesible correspondiente

#### Scenario: Activación de un control iconográfico

- **WHEN** el usuario activa el icono de una acción
- **THEN** el panel SHALL ejecutar la misma llamada y mostrar el mismo resultado y progreso que con el control textual anterior

#### Scenario: Nombre accesible para tecnología de asistencia

- **WHEN** una tecnología de asistencia consulta el nombre de un control de acción
- **THEN** SHALL obtener el nombre de la acción («Arrancar», «Parar», «Reiniciar», «Editar», «Logs» o «Borrar»), no una descripción genérica del icono
