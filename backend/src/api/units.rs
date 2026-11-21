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

use axum::extract::{Path, State};
use serde::Serialize;

use crate::api::response::ApiResponse;
use crate::error::AppError;
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
}

impl From<UnitStatus> for UnitSummary {
    /// Recorta el estado del dominio a lo que se publica.
    fn from(estado: UnitStatus) -> Self {
        Self {
            name: estado.name,
            active_state: estado.active_state,
            sub_state: estado.sub_state,
            unit_file_state: estado.unit_file_state,
        }
    }
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

/// `GET /api/v1/units` — las quadlets del usuario y su estado.
///
/// Sin unidades el listado viene vacío y con `200`: un usuario sin quadlets es un
/// usuario con el panel vacío, no un error.
pub async fn listar(
    State(state): State<AppState>,
) -> Result<ApiResponse<Vec<UnitSummary>>, AppError> {
    let unidades = state.systemd.quadlets().await?;

    Ok(ApiResponse::new(
        unidades.into_iter().map(UnitSummary::from).collect(),
    ))
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
    use std::sync::Arc;

    use axum::http::{Method, StatusCode};

    use crate::systemd::{SystemdClient, SystemdError, UnitAction, UnitName, fake::FakeSystemd};
    use crate::test_support::{DobleQueFalla, entorno, entorno_con, peticion};

    /// Escenario «Hay quadlets instaladas»: el listado trae cada quadlet con sus
    /// cuatro campos y **sin** la procedencia, que no se publica.
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

        assert_eq!(
            unidades[0]
                .as_object()
                .expect("una unidad es un objeto")
                .len(),
            4,
            "el listado publica cuatro campos y ninguno más: {}",
            unidades[0]
        );
        assert!(
            unidades[0].get("origin").is_none(),
            "la procedencia no se publica: {}",
            unidades[0]
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
