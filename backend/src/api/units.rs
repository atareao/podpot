//! Las rutas de las unidades: el listado y las acciones de control.
//!
//! Las dos rutas traducen estado del dominio a JSON de la casa, y nada más: la
//! decisión de **qué** es una quadlet la tomó [`crate::systemd`] —con el cliente
//! real o con el doble, que comparten la regla—, y la traducción de errores de
//! D-Bus a HTTP la hizo [`crate::error`] antes de llegar aquí. Este módulo no
//! vuelve a decidir ninguna de las dos cosas.
//!
//! Las respuestas son más estrechas que el dominio a propósito: el listado no
//! publica la procedencia de la unidad —una ruta del sistema de ficheros del
//! usuario— y una acción no repite el estado entero, que quien preguntó ya tiene.

use std::collections::HashMap;

use axum::extract::{Path, State};
use serde::Serialize;

use crate::api::response::ApiResponse;
use crate::error::AppError;
use crate::quadlet::QuadletKind;
use crate::quadlet::groups::load_groups_para;
use crate::quadlet::stack::stack_name_with_groups;
use crate::state::AppState;
use crate::systemd::{ActiveState, UnitAction, UnitFileState, UnitName, UnitStatus};

/// Una unidad del listado.
#[derive(Debug, Serialize)]
pub struct UnitSummary {
    /// El nombre **completo** de la unidad (`centinela-db.service`), que es lo
    /// que identifica a una quadlet en el gestor: podpot no deriva el nombre del
    /// fichero ni lo acorta.
    name: UnitName,
    /// Si está viva, parada o rota.
    active_state: ActiveState,
    /// El subestado (`running`, `dead`, `exited`, …), tal cual lo informa systemd.
    sub_state: String,
    /// De dónde viene: `generated` para una quadlet.
    unit_file_state: UnitFileState,
    /// El **nombre de fichero** de la quadlet (`web.container`): la identidad con
    /// la que operan `PATCH`/`DELETE /api/v1/quadlets/{name}`. Es el basename de
    /// la procedencia; la ruta completa **no** se publica.
    file_name: String,
    /// El tipo derivado de la extensión del fichero, o `null` si la extensión no
    /// corresponde a un tipo que podpot sepa crear.
    kind: Option<QuadletKind>,
    /// El identificador del stack, derivado de la extensión del fichero y del
    /// `kind` cuando concuerdan, con el nombre de la unidad como fallback. No
    /// depende del estado en vivo.
    stack: String,
}

impl From<UnitStatus> for UnitSummary {
    /// Recorta el estado del dominio a lo que se publica: sin grupos es el
    /// mismo mapper, con el mapa vacío.
    fn from(estado: UnitStatus) -> Self {
        from_con_grupos(estado, &HashMap::new())
    }
}

/// Convierte un `UnitStatus` en `UnitSummary` aplicando el mapeo de grupos.
///
/// Una unidad sin procedencia no es una quadlet, pero si llega aquí se publica
/// con `file_name` vacío y se avisa: un `debug_assert!` haría panificar el
/// servidor por un dato que systemd puede cambiar sin avisar.
pub(crate) fn from_con_grupos(estado: UnitStatus, grupos: &HashMap<String, String>) -> UnitSummary {
    let (file_name, kind, stack) = match estado.file_name() {
        Some(nombre) => {
            // El tipo sale de la extensión del nombre de fichero, y sólo de ella:
            // una extensión que podpot no sepa crear (`.kube`, `.quadlet`, …)
            // deja el `kind` en `null` sin descartar la unidad del listado.
            let kind = std::path::Path::new(nombre)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .and_then(|extension| QuadletKind::from_extension(extension).ok());
            let stack = stack_name_with_groups(Some(nombre), kind, estado.name.as_str(), grupos);

            (nombre.to_owned(), kind, stack)
        }
        None => {
            tracing::warn!(
                unidad = %estado.name,
                "una unidad sin procedencia llegó al listado; se publica sin file_name"
            );
            let stack = stack_name_with_groups(None, None, estado.name.as_str(), grupos);
            (String::new(), None, stack)
        }
    };

    UnitSummary {
        name: estado.name,
        active_state: estado.active_state,
        sub_state: estado.sub_state,
        unit_file_state: estado.unit_file_state,
        file_name,
        kind,
        stack,
    }
}

/// `GET /api/v1/units` — las quadlets del usuario y su estado.
///
/// Sin unidades el listado viene vacío y con `200`: un usuario sin quadlets es un
/// usuario con el panel vacío, no un error.
pub async fn listar(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<UnitSummary>>, AppError> {
    let unidades = state.systemd.quadlets().await?;
    let grupos = load_groups_para(&unidades, &state.config);

    Ok(ApiResponse::new(
        unidades
            .into_iter()
            .map(|u| from_con_grupos(u, &grupos))
            .collect(),
    ))
}

/// Lo que devuelve una acción de control.
///
/// Es más estrecho que [`UnitSummary`] porque la acción ya sabe de qué unidad se
/// trata y no necesita el subestado: lo único que hay que contestar es en qué
/// estado quedó.
#[derive(Debug, Serialize)]
pub struct UnitActionOutcome {
    /// El nombre completo de la unidad accionada.
    name: UnitName,
    /// El estado en el que quedó tras completarse el job.
    active_state: ActiveState,
}

impl From<UnitStatus> for UnitActionOutcome {
    fn from(estado: UnitStatus) -> Self {
        Self {
            name: estado.name,
            active_state: estado.active_state,
        }
    }
}

/// `POST /api/v1/units/{name}/{action}` — arranca, para o reinicia una unidad.
///
/// El orden de las comprobaciones es el de la petición: primero el nombre —que
/// tiene que ser un nombre de unidad, o es un `400`— y después la acción. Una
/// acción que no existe no es una entrada mal formada sino una ruta que no está,
/// así que contesta `404` como cualquier otro recurso desconocido, y dice cuáles
/// son las que sí.
pub async fn accionar(
    State(state): State<AppState>,
    Path((nombre, accion)): Path<(String, String)>,
) -> Result<ApiResponse<UnitActionOutcome>, AppError> {
    let unidad = UnitName::parse(&nombre)?;

    let accion = UnitAction::parse(&accion).ok_or_else(|| {
        AppError::NotFound(format!(
            "la acción `{accion}` no existe: se esperaba `start`, `stop` o `restart`"
        ))
    })?;

    // La acción vuelve cuando el job de systemd ha terminado, así que el estado
    // que se publica es el que quedó, no el que se pidió.
    let estado = state.systemd.action(&unidad, accion).await?;

    Ok(ApiResponse::new(UnitActionOutcome::from(estado)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write;
    use std::sync::Arc;

    use axum::http::{Method, StatusCode};
    use tempfile::TempDir;

    use super::{UnitSummary, from_con_grupos};
    use crate::systemd::{
        ActiveState, SystemdClient, SystemdError, UnitAction, UnitFileState, UnitName, UnitOrigin,
        UnitStatus, fake::FakeSystemd,
    };
    use crate::test_support::{DobleQueFalla, entorno, entorno_con, peticion};

    /// Escenario «Hay quadlets instaladas»: el listado trae cada quadlet con sus
    /// campos y **sin** la procedencia, que no se publica.
    #[tokio::test]
    async fn el_listado_trae_las_quadlets_con_su_estado() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        assert_eq!(status, StatusCode::OK);

        let unidades = cuerpo["data"]
            .as_array()
            .expect("el listado es un array de unidades");

        assert_eq!(
            unidades.len(),
            2,
            "se esperaban las dos quadlets de muestra"
        );
        assert_eq!(unidades[0]["name"], "podpot-demo-db.service");
        assert_eq!(unidades[0]["active_state"], "active");
        assert_eq!(unidades[0]["sub_state"], "running");
        assert_eq!(unidades[0]["unit_file_state"], "generated");
        assert_eq!(unidades[0]["file_name"], "podpot-demo-db.container");
        assert_eq!(unidades[0]["kind"], "container");
        assert_eq!(unidades[0]["stack"], "podpot");

        assert_eq!(
            unidades[0]
                .as_object()
                .expect("una unidad es un objeto")
                .len(),
            7,
            "el listado publica siete campos y ninguno más: {}",
            unidades[0]
        );
        assert!(
            unidades[0].get("origin").is_none(),
            "la procedencia no se publica: {}",
            unidades[0]
        );
    }

    /// Contrato «GET /api/v1/units»: el mapper que alimenta el listado serializa
    /// el `stack` derivado, sin depender de systemd ni del handler HTTP.
    #[test]
    fn el_mapper_publica_el_stack_en_el_json_de_la_unidad() {
        let unidad = |nombre: &str, origen: &str| UnitStatus {
            name: UnitName::sin_validar(nombre),
            active_state: ActiveState::Active,
            sub_state: "running".to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde(origen),
        };

        let principal = UnitSummary::from(unidad(
            "foo.service",
            "/home/lorenzo/.config/containers/systemd/foo.container",
        ));
        let auxiliar = UnitSummary::from(unidad(
            "foo-volume.service",
            "/home/lorenzo/.config/containers/systemd/foo-volume.volume",
        ));
        let sin_tipo = UnitSummary::from(unidad(
            "algo.service",
            "/home/lorenzo/.config/containers/systemd/algo.quadlet",
        ));

        let json_principal = serde_json::to_value(&principal).expect("la unidad se serializa");
        let json_auxiliar = serde_json::to_value(&auxiliar).expect("la unidad se serializa");
        let json_sin_tipo = serde_json::to_value(&sin_tipo).expect("la unidad se serializa");

        assert_eq!(json_principal["stack"], "foo");
        assert_eq!(json_auxiliar["stack"], "foo");
        assert_eq!(json_sin_tipo["stack"], "algo");

        // `.kube`, `.image` y `.build` no son tipos creables (`kind` es `null`),
        // pero son principales a todos los efectos: el stack sale del stem.
        for (nombre, extension) in [
            ("foo-kube.service", "kube"),
            ("foo-image.service", "image"),
            ("foo-build.service", "build"),
        ] {
            let origen = format!("/home/lorenzo/.config/containers/systemd/foo.{extension}");
            let json = serde_json::to_value(UnitSummary::from(unidad(nombre, &origen)))
                .expect("la unidad se serializa");

            assert!(json["kind"].is_null(), "la extensión no es creable: {json}");
            assert_eq!(
                json["stack"], "foo",
                "el stack no depende del nombre: {json}"
            );
        }
    }

    /// Escenario «Override en el mapper»: el mapa de grupos llega al `stack`
    /// publicado, por `file_name` y por `unit_name`, y se serializa en el JSON.
    #[test]
    fn el_mapper_aplica_el_override_de_grupos_al_json() {
        let unidad = || UnitStatus {
            name: UnitName::sin_validar("atareao-mariadb.service"),
            active_state: ActiveState::Active,
            sub_state: "running".to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde("/fuentes/atareao-mariadb.container"),
        };

        let por_fichero = HashMap::from([(
            "atareao-mariadb.container".to_owned(),
            "db-cluster".to_owned(),
        )]);
        let por_unidad = HashMap::from([(
            "atareao-mariadb.service".to_owned(),
            "monitoring".to_owned(),
        )]);

        for (grupos, esperado) in [(&por_fichero, "db-cluster"), (&por_unidad, "monitoring")] {
            let json = serde_json::to_value(from_con_grupos(unidad(), grupos))
                .expect("la unidad se serializa");
            assert_eq!(json["stack"], esperado, "el override gana: {json}");
            assert_eq!(json["file_name"], "atareao-mariadb.container");
        }

        // Sin mapa, el mismo mapper (y el `From`) caen a la regla base.
        let sin_grupos =
            serde_json::to_value(UnitSummary::from(unidad())).expect("la unidad se serializa");
        assert_eq!(sin_grupos["stack"], "atareao");
    }

    /// Contrato HTTP: un `groups.toml` en el directorio de la procedencia
    /// cambia el `stack` que publica `GET /api/v1/units`.
    #[tokio::test]
    async fn el_listado_publica_el_stack_del_groups_toml_de_la_fuente() {
        let fuente = TempDir::new().expect("directorio fuente");
        let mut fichero =
            std::fs::File::create(fuente.path().join("groups.toml")).expect("crear groups.toml");
        writeln!(fichero, "[mappings]\n\"web.container\" = \"proj-web\"")
            .expect("escribir groups.toml");

        let origen = fuente
            .path()
            .join("web.container")
            .to_string_lossy()
            .into_owned();
        let cliente = Arc::new(FakeSystemd::con([UnitStatus {
            name: UnitName::sin_validar("web.service"),
            active_state: ActiveState::Active,
            sub_state: "running".to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde(&origen),
        }]));

        let entorno = entorno_con(cliente).await;
        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        assert_eq!(cuerpo["data"][0]["stack"], "proj-web");
        assert_eq!(cuerpo["data"][0]["file_name"], "web.container");
    }

    /// Escenario «Estabilidad en vivo»: el estado cambia, la identidad no, y el
    /// stack publicado tiene que permanecer intacto.
    #[test]
    fn el_stack_no_cambia_cuando_cambia_el_estado_en_vivo() {
        let estado = |activa: bool| UnitStatus {
            name: UnitName::sin_validar("foo.service"),
            active_state: if activa {
                ActiveState::Active
            } else {
                ActiveState::Inactive
            },
            sub_state: if activa { "running" } else { "dead" }.to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde("/home/lorenzo/.config/containers/systemd/foo.container"),
        };

        let activa =
            serde_json::to_value(UnitSummary::from(estado(true))).expect("la unidad se serializa");
        let parada =
            serde_json::to_value(UnitSummary::from(estado(false))).expect("la unidad se serializa");

        assert_eq!(activa["stack"], "foo");
        assert_eq!(parada["stack"], activa["stack"]);
    }

    /// Escenario «Listado con la identidad real de cada quadlet»: cada unidad
    /// publica su `file_name` —la identidad con la que operan `PATCH`/`DELETE`— y
    /// su `kind` derivado de la extensión.
    #[tokio::test]
    async fn el_listado_publica_la_identidad_de_cada_quadlet() {
        let entorno = entorno().await;

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        let unidades = cuerpo["data"]
            .as_array()
            .expect("el listado es un array de unidades");

        assert_eq!(unidades[0]["file_name"], "podpot-demo-db.container");
        assert_eq!(unidades[0]["kind"], "container");
        assert_eq!(unidades[1]["file_name"], "podpot-demo-web.container");
        assert_eq!(unidades[1]["kind"], "container");
    }

    /// Escenario «La ruta no se publica»: el listado lleva el nombre de fichero
    /// —un basename— pero nunca la ruta de la que systemd la generó.
    #[tokio::test]
    async fn el_listado_no_publica_la_ruta_del_sistema_de_ficheros() {
        let entorno = entorno().await;

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        let unidades = cuerpo["data"]
            .as_array()
            .expect("el listado es un array de unidades");

        for unidad in unidades {
            assert!(
                unidad.get("origin").is_none(),
                "la unidad publicó su procedencia: {unidad}"
            );

            let file_name = unidad["file_name"]
                .as_str()
                .expect("cada quadlet publica su nombre de fichero");
            assert!(
                !file_name.contains('/'),
                "el `file_name` tiene que ser un basename, no una ruta: {file_name}"
            );
        }

        let texto = cuerpo.to_string();
        assert!(
            !texto.contains("/home/") && !texto.contains("containers/systemd"),
            "el listado publicó una ruta del sistema de ficheros: {texto}"
        );
    }

    /// Una quadlet con una extensión que podpot **no** sabe crear se lista igual,
    /// con su `file_name`, pero sin `kind`: la extensión no es un tipo conocido.
    #[tokio::test]
    async fn una_quadlet_con_extension_desconocida_no_tiene_kind() {
        let entorno = entorno_con(Arc::new(FakeSystemd::con([UnitStatus {
            name: UnitName::sin_validar("algo.service"),
            active_state: ActiveState::Active,
            sub_state: "running".to_owned(),
            unit_file_state: UnitFileState::Generated,
            origin: UnitOrigin::desde("/home/lorenzo/.config/containers/systemd/algo.quadlet"),
        }])))
        .await;

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        let unidades = cuerpo["data"]
            .as_array()
            .expect("el listado es un array de unidades");

        assert_eq!(unidades[0]["file_name"], "algo.quadlet");
        assert!(
            unidades[0]["kind"].is_null(),
            "una extensión desconocida no tiene tipo: {}",
            unidades[0]
        );
    }

    /// Escenario «El doble sigue construyendo quadlets con origen»: las quadlets
    /// de muestra se exponen con el `file_name` de su fichero.
    #[tokio::test]
    async fn las_quadlets_del_doble_se_exponen_con_su_file_name() {
        let cliente: Arc<dyn SystemdClient> = Arc::new(FakeSystemd::demo());

        let listado = cliente.quadlets().await.expect("el doble no falla");

        let identidades: Vec<(&str, Option<&str>)> = listado
            .iter()
            .map(|unidad| (unidad.name.as_str(), unidad.file_name()))
            .collect();

        assert_eq!(
            identidades,
            [
                ("podpot-demo-db.service", Some("podpot-demo-db.container")),
                ("podpot-demo-web.service", Some("podpot-demo-web.container")),
            ]
        );
    }

    /// El listado viene ordenado por nombre aunque el gestor no lo ordene: un
    /// panel que cambia el orden entre dos peticiones parece roto.
    #[tokio::test]
    async fn el_listado_viene_ordenado_por_nombre() {
        let entorno = entorno().await;

        let (_, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        let nombres: Vec<&str> = cuerpo["data"]
            .as_array()
            .expect("el listado es un array")
            .iter()
            .map(|unidad| unidad["name"].as_str().unwrap_or_default())
            .collect();

        assert_eq!(
            nombres,
            ["podpot-demo-db.service", "podpot-demo-web.service"]
        );
    }

    /// Escenario «No hay quadlets»: `200` con el listado vacío, no un error.
    #[tokio::test]
    async fn sin_quadlets_el_listado_viene_vacio() {
        let entorno = entorno_con(Arc::new(FakeSystemd::new())).await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"], serde_json::json!([]));
    }

    /// Escenario «Arrancar una unidad inactiva»: se completa y la unidad queda
    /// activa. La respuesta es la estrecha: nombre y estado.
    #[tokio::test]
    async fn arrancar_una_unidad_contesta_el_estado_en_el_que_queda() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/podpot-demo-web.service/start",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cuerpo["data"]["name"], "podpot-demo-web.service");
        assert_eq!(cuerpo["data"]["active_state"], "active");
        assert_eq!(
            cuerpo["data"]
                .as_object()
                .expect("el resultado es un objeto")
                .len(),
            2,
            "la respuesta de una acción es más estrecha que el estado: {cuerpo}"
        );
    }

    /// Parar deja la unidad parada, y es lo que se contesta.
    #[tokio::test]
    async fn parar_una_unidad_contesta_que_queda_parada() {
        let entorno = entorno().await;

        let (_, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/podpot-demo-db.service/stop",
        )
        .await;

        assert_eq!(cuerpo["data"]["active_state"], "inactive");
    }

    /// Escenario «Unidad inexistente»: `404` nombrando la unidad.
    #[tokio::test]
    async fn accionar_una_unidad_que_no_existe_da_404() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/fantasma.service/start",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "unit_not_found");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("fantasma.service"),
            "el error tiene que nombrar la unidad: {cuerpo}"
        );
    }

    /// Una acción que no existe es una ruta que no está: `404` con la envoltura y
    /// diciendo cuáles son las que sí.
    #[tokio::test]
    async fn una_accion_desconocida_da_404_diciendo_cuales_valen() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/podpot-demo-db.service/saltar",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(cuerpo["error"]["code"], "not_found");

        let mensaje = cuerpo["error"]["message"].as_str().unwrap_or_default();
        for valida in ["start", "stop", "restart"] {
            assert!(
                mensaje.contains(valida),
                "el 404 tiene que decir qué acciones valen: {mensaje}"
            );
        }
    }

    /// Y un nombre que no es un nombre de unidad es una entrada mal formada, no un
    /// recurso ausente: `400`.
    #[tokio::test]
    async fn un_nombre_que_no_es_de_unidad_da_400() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/lo-que-sea/start",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "bad_request");
    }

    /// La comprobación del nombre va antes que la de la acción: una ruta con las
    /// dos cosas mal dichas se queja del nombre, que es lo primero que falta.
    #[tokio::test]
    async fn el_nombre_se_comprueba_antes_que_la_accion() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/lo-que-sea/saltar",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(cuerpo["error"]["code"], "bad_request");
    }

    /// Un método que no está declarado sobre una ruta que sí existe contesta `405`
    /// con la envoltura, también en las rutas de unidades.
    #[tokio::test]
    async fn un_metodo_no_permitido_sobre_las_unidades_da_405() {
        let entorno = entorno().await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::DELETE, "/api/v1/units").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");

        // Y el listado de unidades no se puede accionar con un `POST` a secas:
        // la acción va en la ruta.
        let (status, cuerpo) = peticion(&entorno.app(), Method::POST, "/api/v1/units").await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(cuerpo["error"]["code"], "method_not_allowed");
    }

    /// Un job que no lleva la unidad a su estado es un `502` —el fallo es del
    /// gestor, no de la petición—, y no publica el resultado crudo.
    #[tokio::test]
    async fn una_operacion_que_falla_da_502_sin_publicar_el_resultado() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| {
            SystemdError::OperationFailed {
                unit: "rota.service".to_owned(),
                action: "start",
                result: "resources".to_owned(),
            }
        })))
        .await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/rota.service/start",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(cuerpo["error"]["code"], "systemd_operation_failed");
        assert!(
            !cuerpo.to_string().contains("resources"),
            "el 502 publicó el resultado crudo del job: {cuerpo}"
        );
    }

    /// Una operación que no termina en plazo es un `504`.
    #[tokio::test]
    async fn una_operacion_que_no_termina_en_plazo_da_504() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| SystemdError::TimedOut {
            unit: "lenta.service".to_owned(),
        })))
        .await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/lenta.service/start",
        )
        .await;

        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(cuerpo["error"]["code"], "systemd_timeout");
    }

    /// Y un gestor que no contesta es un `503`, sin object paths ni texto de la
    /// biblioteca —también por el manejador, no sólo en la traducción suelta—.
    #[tokio::test]
    async fn un_gestor_que_no_contesta_da_503_sin_filtrar_detalles() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| SystemdError::Transport {
            causa: Box::new(std::io::Error::other(
                "did not receive a reply for /org/freedesktop/systemd1/unit/x_2eservice",
            )),
        })))
        .await;

        let (status, cuerpo) = peticion(&entorno.app(), Method::GET, "/api/v1/units").await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(cuerpo["error"]["code"], "systemd_unavailable");

        let texto = cuerpo.to_string();
        assert!(
            !texto.contains("systemd1"),
            "el 503 publicó un object path: {texto}"
        );
        assert!(
            !texto.contains("did not receive a reply"),
            "el 503 publicó el texto de la biblioteca: {texto}"
        );
    }

    /// Y un gestor que rechaza la operación —unidad enmascarada, no cargable— da
    /// un `502`: contestó, pero dijo que no. No es un `503`, que es cuando no
    /// contesta, ni un `404`, que es cuando la unidad no está.
    #[tokio::test]
    async fn un_gestor_que_rechaza_la_operacion_da_502_sin_filtrar_detalles() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| {
            SystemdError::UnitRejected {
                unit: "masked.service".to_owned(),
                causa: Box::new(std::io::Error::other(
                    "org.freedesktop.systemd1.UnitMasked: Unit masked.service is masked",
                )),
            }
        })))
        .await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/masked.service/start",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(cuerpo["error"]["code"], "systemd_unit_rejected");
        assert!(
            cuerpo["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("masked.service"),
            "el 502 tiene que nombrar la unidad: {cuerpo}"
        );

        let texto = cuerpo.to_string();
        assert!(
            !texto.contains("UnitMasked"),
            "el 502 publicó el texto del gestor: {texto}"
        );
    }

    /// La ruta de la acción también pregunta al gestor por el estado de la unidad:
    /// un fallo al leerlo sale por el mismo camino que el de la acción.
    #[tokio::test]
    async fn el_estado_de_una_unidad_que_falla_al_leerse_da_503() {
        let entorno = entorno_con(Arc::new(DobleQueFalla::con(|| SystemdError::Transport {
            causa: Box::new(std::io::Error::other("el bus se cayó")),
        })))
        .await;

        let (status, cuerpo) = peticion(
            &entorno.app(),
            Method::POST,
            "/api/v1/units/loquesea.service/start",
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(cuerpo["error"]["code"], "systemd_unavailable");
    }

    /// El doble de la casa sigue siendo un `SystemdClient` de verdad: las pruebas
    /// de arriba inyectan `Arc<dyn SystemdClient>`, y ésa es la forma que tiene
    /// `main` de componer el proceso.
    #[tokio::test]
    async fn el_entorno_inyecta_el_doble_por_el_trazo_del_cliente() {
        let cliente: Arc<dyn SystemdClient> = Arc::new(FakeSystemd::demo());

        assert_eq!(
            cliente.quadlets().await.expect("el doble no falla").len(),
            2
        );

        // Y accionar lo que el doble no conoce sigue siendo un error tipado.
        let fantasma = UnitName::sin_validar("fantasma.service");
        assert!(matches!(
            cliente.action(&fantasma, UnitAction::Start).await,
            Err(SystemdError::UnitNotFound { .. })
        ));
    }
}
