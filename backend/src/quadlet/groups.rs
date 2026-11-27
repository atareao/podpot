//! Carga y fusión de `groups.toml` para el mapeo de stacks.
//!
//! Cada directorio que sea **fuente real** de quadlets puede contener un fichero
//! `groups.toml` con una tabla `[mappings]` que asocia nombres de fichero
//! (`file_name`) a nombres de stack (`stack_name`). El override permite anular o
//! redefinir el stack derivado de la extensión.
//!
//! ```toml
//! [mappings]
//! "web.container" = "web-custom"
//! "db.volume" = "db-overridden"
//! ```
//!
//! Las claves son nombres de fichero (basename) o nombres de unidad, y los
//! valores son los nombres de stack que sustituyen al derivado.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::Config;
use crate::systemd::UnitStatus;

/// Configuración de grupos cargada de un `groups.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupsConfig {
    /// Mapeo de nombres de fichero o unidad a nombres de stack.
    #[serde(default, rename = "mappings")]
    pub mappings: HashMap<String, String>,
}

impl GroupsConfig {
    /// Carga `groups.toml` de cada directorio, en orden; el último gana.
    ///
    /// Los directorios ausentes, ilegibles o mal formados se ignoran con una
    /// traza: un `groups.toml` roto no debe romper el listado.
    pub fn load_groups(dirs: &[PathBuf]) -> HashMap<String, String> {
        let mut merged = HashMap::new();

        for dir in dirs {
            let ruta = dir.join("groups.toml");
            if !ruta.exists() {
                continue;
            }

            match GroupsConfig::leer_desde(&ruta) {
                Ok(config) => merged.extend(config.mappings),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        ruta = %ruta.display(),
                        "no se pudo cargar groups.toml; se ignoran sus mapeos"
                    );
                }
            }
        }

        merged
    }

    /// Lee y parsea un fichero `groups.toml` en la ruta dada.
    fn leer_desde(ruta: &Path) -> Result<GroupsConfig, Box<dyn std::error::Error>> {
        let contenido = std::fs::read_to_string(ruta)?;
        let config: GroupsConfig = toml::from_str(&contenido)?;
        Ok(config)
    }
}

/// Directorios que son fuente real de las unidades dadas.
///
/// La procedencia de cada quadlet ([`UnitStatus::origin`]) es la fuente
/// autoritativa: systemd genera la unidad desde el fichero que dice ahí, y es el
/// directorio de ese fichero donde tiene que estar el `groups.toml`. Se añade
/// además [`Config::quadlet_dir`] —de `PODPOT_QUADLET_DIR` o `$HOME`— como base
/// de la creación.
///
/// El directorio de trabajo **no** se consulta nunca: que el proceso se haya
/// arrancado en un directorio con un `groups.toml` accidental no debe cambiar el
/// stack de unidades que no vienen de ahí.
///
/// El resultado es determinista y sin duplicados: la base configurada va primero
/// y las procedencias después, ordenadas; así una procedencia puede anular a la
/// base y el orden entre procedencias no depende del orden del listado.
pub fn source_dirs(unidades: &[UnitStatus], config: &Config) -> Vec<PathBuf> {
    let mut dirs = vec![config.quadlet_dir.clone()];

    dirs.extend(
        unidades
            .iter()
            .filter_map(|unidad| unidad.origin.directorio())
            .collect::<Vec<_>>(),
    );

    // `dedup` exige orden previo. La base se preserva en la primera posición
    // ordenando sólo las procedencias: éstas son las que deben ganar.
    dirs.sort();
    dirs.dedup();

    if let Some(pos) = dirs.iter().position(|dir| dir == &config.quadlet_dir) {
        let base = dirs.remove(pos);
        dirs.insert(0, base);
    }

    dirs
}

/// Carga los grupos de las fuentes reales de un listado de unidades.
///
/// Se llama en cada petición y en cada instantánea SSE: cambiar un `groups.toml`
/// surte efecto en el siguiente evento sin reconectar ni reiniciar.
pub fn load_groups_para(unidades: &[UnitStatus], config: &Config) -> HashMap<String, String> {
    GroupsConfig::load_groups(&source_dirs(unidades, config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::systemd::{ActiveState, UnitFileState, UnitName, UnitOrigin};

    use std::io::Write;
    use tempfile::TempDir;

    /// Configura un `quadlet_dir` propio sin depender del entorno de la máquina.
    fn config_con_quadlet_dir(dir: &Path) -> Config {
        let dir = dir.to_str().expect("ruta UTF-8").to_owned();
        Config::from_source(move |clave| match clave {
            "DATABASE_URL" => Some("sqlite::memory:".to_owned()),
            "HOME" => Some("/home/tester".to_owned()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".to_owned()),
            "PODPOT_QUADLET_DIR" => Some(dir.clone()),
            _ => None,
        })
        .expect("la configuración de prueba es válida")
    }

    /// Escribe un `groups.toml` con un único mapeo en el directorio dado.
    fn escribir_grupos(dir: &Path, clave: &str, valor: &str) {
        let mut fichero =
            std::fs::File::create(dir.join("groups.toml")).expect("crear groups.toml");
        writeln!(fichero, "[mappings]\n\"{clave}\" = \"{valor}\"").expect("escribir groups.toml");
    }

    /// Unidad de prueba con la procedencia indicada.
    fn unidad_en(origen: &str) -> UnitStatus {
        let nombre = Path::new(origen)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .expect("la procedencia tiene nombre");
        UnitStatus {
            name: UnitName::sin_validar(nombre),
            active_state: ActiveState::Active,
            sub_state: "running".to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde(origen),
        }
    }

    /// Escenario «Sin groups.toml»: ninguna fusión, mapa vacío.
    #[test]
    fn sin_groups_toml() {
        let dirs = vec![
            PathBuf::from("/nonexistent/dir1"),
            PathBuf::from("/nonexistent/dir2"),
        ];
        assert!(GroupsConfig::load_groups(&dirs).is_empty());
    }

    /// Escenario «groups.toml con mapeos».
    #[test]
    fn groups_toml_con_mapeo() {
        let texto = r#"[mappings]
        "web.container" = "web-custom"
        "db.volume" = "db-overridden""#;
        let config: GroupsConfig = toml::from_str(texto).expect("parseo exitoso");

        assert_eq!(
            config.mappings.get("web.container"),
            Some(&"web-custom".to_owned())
        );
        assert_eq!(
            config.mappings.get("db.volume"),
            Some(&"db-overridden".to_owned())
        );
    }

    /// Escenario «groups.toml vacío».
    #[test]
    fn groups_toml_vacio() {
        let config: GroupsConfig = toml::from_str("[mappings]").expect("parseo exitoso");
        assert!(config.mappings.is_empty());
    }

    /// Escenario «Fusionar directorios: el último sobrescribe».
    #[test]
    fn fusion_directorios_ultimo_gana() {
        let dir1 = TempDir::new().expect("temp dir");
        let dir2 = TempDir::new().expect("temp dir");
        escribir_grupos(dir1.path(), "web.container", "web-dir1");
        escribir_grupos(dir2.path(), "web.container", "web-dir2");

        let mapa =
            GroupsConfig::load_groups(&[dir1.path().to_path_buf(), dir2.path().to_path_buf()]);

        assert_eq!(
            mapa.get("web.container"),
            Some(&"web-dir2".to_owned()),
            "el último directorio debe ganar"
        );
    }

    /// Escenario «groups.toml mal formado»: se ignora con traza, sin panificar.
    #[test]
    fn groups_toml_mal_formado_se_ignora() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(dir.path().join("groups.toml"), "[mappings").expect("escribir");

        assert!(GroupsConfig::load_groups(&[dir.path().to_path_buf()]).is_empty());
    }

    /// Escenario «Fuentes reales»: salen del origin y de la config, sin CWD.
    #[test]
    fn source_dirs_solo_fuentes_reales() {
        let base = TempDir::new().expect("temp dir");
        let origen = TempDir::new().expect("temp dir");
        let config = config_con_quadlet_dir(base.path());
        let unidades = vec![unidad_en(
            &origen.path().join("web.container").to_string_lossy(),
        )];

        let dirs = source_dirs(&unidades, &config);

        assert_eq!(dirs.len(), 2, "sólo la base y la procedencia: {dirs:?}");
        assert_eq!(dirs[0], base.path(), "la base configurada va primero");
        assert_eq!(dirs[1], origen.path(), "la procedencia va detrás");
        assert!(
            !dirs.iter().any(|dir| dir == Path::new(".")),
            "el CWD no es fuente: {dirs:?}"
        );
    }

    /// Escenario «Unidad sin procedencia»: no añade directorios fantasma.
    #[test]
    fn source_dirs_ignora_unidades_sin_procedencia() {
        let base = TempDir::new().expect("temp dir");
        let config = config_con_quadlet_dir(base.path());
        let mut sin_origen = unidad_en("web.container");
        sin_origen.origin = UnitOrigin::desconocido();

        assert_eq!(source_dirs(&[sin_origen], &config), vec![base.path()]);
    }

    /// Escenario «Carga por procedencia»: el groups.toml del directorio de la
    /// unidad se aplica, aunque el directorio configurado tenga otro mapeo.
    #[test]
    fn load_groups_para_usa_el_directorio_de_la_procedencia() {
        let base = TempDir::new().expect("temp dir");
        let origen = TempDir::new().expect("temp dir");
        escribir_grupos(base.path(), "web.container", "de-la-base");
        escribir_grupos(origen.path(), "web.container", "de-la-procedencia");
        let config = config_con_quadlet_dir(base.path());
        let unidades = vec![unidad_en(
            &origen.path().join("web.container").to_string_lossy(),
        )];

        let mapa = load_groups_para(&unidades, &config);

        assert_eq!(
            mapa.get("web.container"),
            Some(&"de-la-procedencia".to_owned())
        );
    }

    /// Escenario «CWD accidental»: un groups.toml fuera de las fuentes no cuenta.
    #[test]
    fn un_groups_toml_ajeno_a_las_fuentes_no_se_aplica() {
        let base = TempDir::new().expect("temp dir");
        let otro = TempDir::new().expect("temp dir");
        escribir_grupos(otro.path(), "web.container", "no-debe-aparecer");
        let config = config_con_quadlet_dir(base.path());
        let unidades = vec![unidad_en(
            &base.path().join("web.container").to_string_lossy(),
        )];

        assert!(load_groups_para(&unidades, &config).is_empty());
    }
}
