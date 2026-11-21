-- 0003_quadlet_operations.sql — el registro de las operaciones sobre quadlets
-- existentes (edición y borrado).
--
-- Igual que `quadlet_creations`, este fichero **no** guarda el contenido de la
-- quadlet —que vive en el sistema de ficheros—, guarda el hecho de que podpot la
-- editó o la borró. Es la traza que explica «quién tocó esta quadlet», que no se
-- puede reconstruir ni desde systemd ni desde el disco.
--
-- El prefijo numérico entero (`0003_`, no `V003__`) es obligatorio: el migrador
-- embebido de `sqlx` corta el nombre por el primer `_` y lee la primera parte como
-- número.

CREATE TABLE quadlet_operations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    operation   TEXT NOT NULL CHECK (operation IN ('edit', 'delete')),
    file_name   TEXT NOT NULL,
    occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- El registro se lee por fichero y de lo más reciente a lo más antiguo, que es
-- como se explica «qué le pasó a esta quadlet».
CREATE INDEX quadlet_operations_file_name_occurred_at
    ON quadlet_operations (file_name, occurred_at DESC);
