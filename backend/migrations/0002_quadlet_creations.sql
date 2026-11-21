-- 0002_quadlet_creations.sql — el registro de las quadlets creadas.
--
-- La quadlet en sí vive en el sistema de ficheros: este fichero **no** guarda su
-- contenido, guarda el hecho de que podpot la creó. Es la traza que explica «de
-- dónde salió esta unidad», que no se puede reconstruir desde systemd.
--
-- El prefijo numérico entero (`0002_`, no `V002__`) es obligatorio: el migrador
-- embebido de `sqlx` corta el nombre por el primer `_` y lee la primera parte como
-- número.

CREATE TABLE quadlet_creations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    file_name   TEXT NOT NULL,
    unit_name   TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('container', 'volume', 'network', 'pod')),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- El registro se lee por fichero y de lo más reciente a lo más antiguo, que es
-- como se explica «quién creó esta quadlet».
CREATE INDEX quadlet_creations_file_name_created_at
    ON quadlet_creations (file_name, created_at DESC);
