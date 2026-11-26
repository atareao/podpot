//!La derivación del stack a partir de la identidad de la quadlet.
//!
//!El stack es el nombre con el que el panel agrupa las unidades generadas por
//!un mismo fichero base. Se calcula aquí, como una función pura de la
//!identidad (`file_name`, `kind`) y del nombre de la unidad: nunca depende del
//!estado en vivo, de modo que un evento de systemd no puede cambiar de grupo a
//!una unidad.

use crate::quadlet::QuadletKind;
use crate::systemd::QUADLET_EXTENSIONS;

/// Deriva el nombre del stack de una unidad.
///
/// La extensión **real** del fichero (`QUADLET_EXTENSIONS`) fija el stem y,
/// cuando no hay un `kind` tipado, también la categoría: `volume` y `network`
/// son auxiliares y el resto —incluidos `.kube`, `.image` y `.build`, que
/// podpot no sabe crear— son principales. Si `kind` existe tiene que concordar
/// con esa extensión; de lo contrario se cae al nombre de la unidad sin
/// `.service`, como se hace también para una extensión desconocida o un
/// `file_name` ausente. El resultado nunca es vacío y conserva la grafía.
pub fn stack_name(file_name: Option<&str>, kind: Option<QuadletKind>, unit_name: &str) -> String {
    let fallback = || {
        let sin_sufijo = unit_name.strip_suffix(".service").unwrap_or(unit_name);
        if sin_sufijo.is_empty() {
            unit_name
        } else {
            sin_sufijo
        }
    };

    let Some((stem, extension)) = file_name.and_then(stem_y_extension) else {
        return fallback().to_owned();
    };
    if kind.is_some_and(|kind| kind.extension() != extension) {
        return fallback().to_owned();
    }

    let categoria = match kind {
        Some(QuadletKind::Volume) => Categoria::Auxiliar("-volume"),
        Some(QuadletKind::Network) => Categoria::Auxiliar("-network"),
        Some(QuadletKind::Container | QuadletKind::Pod) => Categoria::Principal,
        None => categoria_de_extension(extension),
    };
    let stack = match categoria {
        Categoria::Principal => stem,
        Categoria::Auxiliar(sufijo) => recortar_auxiliar(stem, sufijo),
    };

    if stack.is_empty() {
        fallback().to_owned()
    } else {
        stack.to_owned()
    }
}

/// Categoría de una unidad a efectos de agrupación.
#[derive(Clone, Copy)]
enum Categoria {
    /// Conserva el stem completo (`foo-volume.container` da `foo-volume`).
    Principal,
    /// Recorta su sufijo (`foo-volume.volume` da `foo`).
    Auxiliar(&'static str),
}

/// Devuelve el stem y la extensión sin el punto sólo si ésta es de quadlet.
fn stem_y_extension(file_name: &str) -> Option<(&str, &str)> {
    QUADLET_EXTENSIONS.iter().find_map(|extension| {
        file_name
            .strip_suffix(extension)
            .zip(extension.strip_prefix('.'))
    })
}

/// Clasifica una extensión reconocida. Las auxiliares son `volume` y `network`;
/// `container`, `pod`, `kube`, `image` y `build` son principales.
fn categoria_de_extension(extension: &str) -> Categoria {
    match extension {
        "volume" => Categoria::Auxiliar("-volume"),
        "network" => Categoria::Auxiliar("-network"),
        _ => Categoria::Principal,
    }
}

/// Quita el sufijo de una unidad auxiliar con cualquier grafía, sin tocar la
/// del prefijo, y sólo si queda algo delante del sufijo.
fn recortar_auxiliar<'a>(stem: &'a str, sufijo: &str) -> &'a str {
    let Some(inicio) = stem.len().checked_sub(sufijo.len()) else {
        return stem;
    };
    if inicio == 0 {
        return stem;
    }

    match stem.get(inicio..) {
        Some(cola) if cola.eq_ignore_ascii_case(sufijo) => stem.get(..inicio).unwrap_or(stem),
        _ => stem,
    }
}

#[cfg(test)]
mod tests {
    use super::stack_name;
    use crate::quadlet::QuadletKind;

    #[test]
    fn principal_y_auxiliares_comparten_stack() {
        let principal = stack_name(
            Some("foo.container"),
            Some(QuadletKind::Container),
            "foo.service",
        );
        let volumen = stack_name(
            Some("foo.volume"),
            Some(QuadletKind::Volume),
            "foo-volume.service",
        );
        let red = stack_name(
            Some("foo.network"),
            Some(QuadletKind::Network),
            "foo-network.service",
        );

        assert_eq!(principal, "foo");
        assert_eq!(volumen, "foo");
        assert_eq!(red, "foo");
    }

    #[test]
    fn auxiliar_huerfano_forma_su_propio_stack() {
        let stack = stack_name(
            Some("backup.volume"),
            Some(QuadletKind::Volume),
            "backup-volume.service",
        );

        assert_eq!(stack, "backup");
    }

    #[test]
    fn sufijo_de_auxiliar_solo_se_recorta_si_queda_prefijo() {
        let sin_prefijo = stack_name(
            Some("-volume.volume"),
            Some(QuadletKind::Volume),
            "-volume-volume.service",
        );
        let con_prefijo = stack_name(
            Some("foo-volume.volume"),
            Some(QuadletKind::Volume),
            "foo-volume-volume.service",
        );

        assert_eq!(sin_prefijo, "-volume");
        assert_eq!(con_prefijo, "foo");
    }

    #[test]
    fn sufijo_en_principal_no_se_recorta() {
        let stack = stack_name(
            Some("foo-volume.container"),
            Some(QuadletKind::Container),
            "foo-volume.service",
        );

        assert_eq!(stack, "foo-volume");
    }

    #[test]
    fn pod_es_principal_y_no_recorta_su_sufijo_de_unidad() {
        let stack = stack_name(Some("foo.pod"), Some(QuadletKind::Pod), "foo-pod.service");

        assert_eq!(stack, "foo");
    }

    #[test]
    fn kind_none_deriva_del_nombre_de_unidad_sin_recortar_sufijos() {
        let sin_tipo = stack_name(Some("algo.quadlet"), None, "Mi-Servicio.service");
        let con_sufijo_de_auxiliar = stack_name(None, None, "algo-volume.service");

        assert_eq!(sin_tipo, "Mi-Servicio");
        assert_eq!(con_sufijo_de_auxiliar, "algo-volume");
    }

    #[test]
    fn sin_file_name_deriva_del_nombre_de_unidad() {
        let con_kind = stack_name(None, Some(QuadletKind::Container), "web.service");
        let sin_kind = stack_name(None, None, "web.service");

        assert_eq!(con_kind, "web");
        assert_eq!(sin_kind, "web");
    }

    #[test]
    fn nunca_devuelve_un_stack_vacio() {
        let sin_file = stack_name(None, None, ".service");
        let stem_vacio = stack_name(Some(".container"), Some(QuadletKind::Container), ".service");

        assert!(!sin_file.is_empty(), "el fallback no puede estar vacío");
        assert!(
            !stem_vacio.is_empty(),
            "un stem vacío tiene que caer al fallback"
        );
        assert_eq!(stem_vacio, sin_file);
    }

    #[test]
    fn preserva_la_grafia_original() {
        let stack = stack_name(
            Some("MiPila.volume"),
            Some(QuadletKind::Volume),
            "MiPila-volume.service",
        );

        assert_eq!(stack, "MiPila");
    }

    #[test]
    fn kube_image_y_build_son_principales_aunque_no_tengan_kind() {
        let kube = stack_name(Some("foo.kube"), None, "foo-kube.service");
        let image = stack_name(Some("foo.image"), None, "foo-image.service");
        let build = stack_name(Some("foo.build"), None, "foo-build.service");

        assert_eq!(kube, "foo");
        assert_eq!(image, "foo");
        assert_eq!(build, "foo");
    }

    #[test]
    fn extension_no_reconocida_cae_al_fallback_aunque_haya_kind() {
        let stack = stack_name(
            Some("algo.quadlet"),
            Some(QuadletKind::Container),
            "otro.service",
        );

        assert_eq!(stack, "otro");
    }

    #[test]
    fn extension_y_kind_incoherentes_caen_al_fallback() {
        let stack = stack_name(
            Some("foo.volume"),
            Some(QuadletKind::Container),
            "otro.service",
        );

        assert_eq!(stack, "otro");
    }

    #[test]
    fn recorte_auxiliar_ignora_mayusculas_y_conserva_la_grafia() {
        let volumen = stack_name(
            Some("MiPila-Volume.volume"),
            Some(QuadletKind::Volume),
            "MiPila-Volume-volume.service",
        );
        let red = stack_name(
            Some("MiPila-Network.network"),
            Some(QuadletKind::Network),
            "MiPila-Network-network.service",
        );
        let sin_prefijo = stack_name(
            Some("-VOLUME.volume"),
            Some(QuadletKind::Volume),
            "-VOLUME-volume.service",
        );

        assert_eq!(volumen, "MiPila");
        assert_eq!(red, "MiPila");
        assert_eq!(sin_prefijo, "-VOLUME");
    }
}
