-- 0001_init.sql — el esquema mínimo de podpot.
--
-- Esta entrega no modela unidades: el estado autoritativo de una quadlet vive en
-- systemd, no en una tabla. Lo que sí es de podpot y tiene que sobrevivir al
-- reinicio son los ajustes del panel y el registro de las acciones de control
-- que se le piden al gestor de systemd.
--
-- El prefijo numérico entero (`0001_`, no `V001__`) no es un capricho: el
-- migrador embebido de `sqlx` corta el nombre del fichero por el primer `_` y
-- lee la primera parte como número, así que `V001__init.sql` no se reconoce.

-- Ajustes del panel: pares clave/valor que no tienen estructura fija. Se guardan
-- como texto porque el esquema no debe opinar sobre el formato de cada valor.
CREATE TABLE settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Registro de acciones de control: qué se le pidió a systemd, sobre qué unidad y
-- cómo terminó. Es la traza que explica «por qué esta unidad se reinició», que
-- no se puede reconstruir desde systemd.
CREATE TABLE action_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    unit        TEXT NOT NULL,
    action      TEXT NOT NULL CHECK (action IN ('start', 'stop', 'restart')),
    result      TEXT NOT NULL,
    occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- El registro se lee casi siempre por unidad y de lo más reciente a lo más
-- antiguo.
CREATE INDEX action_log_unit_occurred_at ON action_log (unit, occurred_at DESC);
