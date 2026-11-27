//!La derivación del stack a partir de la identidad de la quadlet.
//!
//!El stack es el nombre con el que el panel agrupa las unidades generadas por
//!un mismo fichero base. Se calcula aquí, como una función pura de la
//!identidad (`file_name`, `kind`) y del nombre de la unidad: nunca depende del
//!estado en vivo, de modo que un evento de systemd no puede cambiar de grupo a
//!una unidad.

use std::collections::HashMap;

use crate::quadlet::QuadletKind;
use crate::systemd::QUADLET_EXTENSIONS;

/// Deriva el nombre del stack de una unidad.
///
/// Regla base: el stack es el primer segmento del nombre base del fichero
/// (stem) antes del primer `-`, conservando la grafía original. Si no hay
/// `-`, el stack es el stem completo.
///
/// Si no hay `file_name`, o la extensión no es de quadlet, o el `kind` no
/// concuerda con la extensión, se usa el nombre de la unidad sin `.service`
/// y se le aplica la misma regla de prefijo. El resultado nunca es vacío.
pub fn stack_name(file_name: Option<&str>, kind: Option<QuadletKind>, unit_name: &str) -> String {
    let fallback = || stack_desde_unit_name(unit_name);

    let Some((stem, extension)) = file_name.and_then(stem_y_extension) else {
        return fallback();
    };
    if kind.is_some_and(|kind| kind.extension() != extension) {
        return fallback();
    }
    if stem.is_empty() {
        return fallback();
    }

    prefijo_de(stem)
}

/// Deriva el nombre del stack de una unidad, aplicando el mapeo de grupos si
/// existe. Si `file_name` o `unit_name` tienen una entrada en `groups`,
/// ese nombre de stack reemplaza al derivado.
pub fn stack_name_with_groups(
    file_name: Option<&str>,
    kind: Option<QuadletKind>,
    unit_name: &str,
    groups: &HashMap<String, String>,
) -> String {
    if let Some(stack) = file_name
        .and_then(|nombre| groups.get(nombre))
        .filter(|stack| es_valido(stack))
    {
        return stack.clone();
    }
    if let Some(stack) = groups.get(unit_name).filter(|stack| es_valido(stack)) {
        return stack.clone();
    }
    stack_name(file_name, kind, unit_name)
}

/// Un override sólo sustituye al stack derivado si dice algo: ni vacío ni
/// formado sólo por espacios. De lo contrario se caería al `""` que promete no
/// devolver este módulo.
fn es_valido(stack: &str) -> bool {
    !stack.trim().is_empty()
}

/// Primer segmento antes del primer `-`, conservando la grafía original.
fn prefijo_de(nombre: &str) -> String {
    if nombre.is_empty() {
        return nombre.to_owned();
    }
    match nombre.split_once('-') {
        Some((prefijo, _)) if !prefijo.is_empty() => prefijo.to_owned(),
        _ => nombre.to_owned(),
    }
}

/// Fallback para cuando no hay `file_name`: usa `unit_name` sin `.service`
/// y le aplica la regla de prefijo, pero nunca devuelve vacío.
fn stack_desde_unit_name(unit_name: &str) -> String {
    let nombre = unit_name.strip_suffix(".service").unwrap_or(unit_name);
    if nombre.is_empty() {
        unit_name.to_owned()
    } else {
        prefijo_de(nombre)
    }
}

/// Devuelve el stem y la extensión sin el punto sólo si ésta es de quadlet.
fn stem_y_extension(file_name: &str) -> Option<(&str, &str)> {
    QUADLET_EXTENSIONS.iter().find_map(|extension| {
        file_name
            .strip_suffix(extension)
            .zip(extension.strip_prefix('.'))
    })
}

#[cfg(test)]
mod tests {
    use super::{stack_name, stack_name_with_groups};
    use crate::quadlet::QuadletKind;
    use std::collections::HashMap;

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

        assert_eq!(stack, "foo");
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

        assert_eq!(sin_tipo, "Mi");
        assert_eq!(con_sufijo_de_auxiliar, "algo");
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

    #[test]
    fn regla_base_con_prefijo() {
        assert_eq!(
            stack_name(
                Some("atareao-mariadb.container"),
                Some(QuadletKind::Container),
                "atareao-mariadb.service",
            ),
            "atareao"
        );
        assert_eq!(
            stack_name(
                Some("centinela-app.container"),
                Some(QuadletKind::Container),
                "centinela-app.service",
            ),
            "centinela"
        );
    }

    #[test]
    fn regla_base_sin_prefijo() {
        assert_eq!(
            stack_name(
                Some("standalone.container"),
                Some(QuadletKind::Container),
                "standalone.service",
            ),
            "standalone"
        );
    }

    #[test]
    fn fallback_a_unit_name_con_prefijo() {
        assert_eq!(
            stack_name(None, Some(QuadletKind::Container), "centinela-app.service"),
            "centinela"
        );
    }

    #[test]
    fn override_por_file_name_gana_sobre_la_regla_base() {
        let grupos = HashMap::from([(
            "atareao-mariadb.container".to_owned(),
            "db-cluster".to_owned(),
        )]);
        let stack = stack_name_with_groups(
            Some("atareao-mariadb.container"),
            Some(QuadletKind::Container),
            "atareao-mariadb.service",
            &grupos,
        );

        assert_eq!(stack, "db-cluster");
    }

    #[test]
    fn override_vacio_o_espacios_cae_a_la_regla_base() {
        let grupos = HashMap::from([
            ("vacio.container".to_owned(), "".to_owned()),
            ("espacios.container".to_owned(), "   \t ".to_owned()),
        ]);

        for nombre in ["vacio.container", "espacios.container"] {
            let stack = stack_name_with_groups(
                Some(nombre),
                Some(QuadletKind::Container),
                &nombre.replace(".container", ".service"),
                &grupos,
            );
            assert_eq!(stack, nombre.trim_end_matches(".container"));
        }
    }

    #[test]
    fn override_por_unit_name_gana_sobre_la_regla_base() {
        let grupos = HashMap::from([("centinela-app.service".to_owned(), "monitoring".to_owned())]);
        let stack = stack_name_with_groups(
            Some("centinela-app.container"),
            Some(QuadletKind::Container),
            "centinela-app.service",
            &grupos,
        );

        assert_eq!(stack, "monitoring");
    }
}
