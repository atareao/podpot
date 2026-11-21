//! El doble de systemd: un cliente en memoria, para desarrollo y pruebas.
//!
//! Existe por dos razones concretas. La primera, que las pruebas no pueden
//! depender de que la máquina que compila tenga un systemd de usuario en pie —ni
//! de que la cuota de D-Bus del contenedor esté bien montada—: con el doble, el
//! contrato del trait se prueba sin bus. La segunda, que permite arrancar podpot
//! en un sitio sin systemd (`PODPOT_SYSTEMD=fake`) y ver el panel en marcha.
//!
//! Lo que **no** es: el doble no conoce el sistema. Sus datos son los que se le
//! hayan dado con [`FakeSystemd::con`] y nada más, así que no sirve para
//! observar systemd ni para desplegar. Por eso comparte con el cliente real la
//! regla del dominio —[`UnitStatus::is_quadlet`]— en vez de decidir por su
//! cuenta qué es una quadlet: si el doble contestara otra cosa que el cliente, las
//! pruebas estarían midiendo el doble y no la aplicación.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{
    ActiveState, ProgresoDeJob, ProgresoDeTrabajos, SystemdClient, SystemdError, UnitAction,
    UnitFileState, UnitName, UnitOrigin, UnitStatus, UnitWatcher,
};

/// Cuántos avisos de cambio caben antes de perder el más viejo.
///
/// Perder un aviso no rompe nada —lo único que provoca es un recálculo—, pero el
/// canal no puede ser de capacidad cero: con cero no habría avisos y el panel se
/// quedaría congelado sin que nada lo delatara.
const CAPACIDAD_DE_AVISOS: usize = 16;

/// Cuántos eventos del ciclo de vida de un job caben antes de perder el más viejo.
///
/// Perder uno no rompe el panel —los jobs en curso caben de sobra en esta
/// capacidad, y el estado final llega igual por el canal de unidades—, pero con
/// cero no habría ningún evento y el canal sería un decorado.
const CAPACIDAD_DE_TRABAJOS: usize = 16;

/// Un cliente de systemd que no habla con systemd.
#[derive(Debug)]
pub struct FakeSystemd {
    /// Las unidades que conoce, por nombre.
    ///
    /// Es un `BTreeMap` y no un `HashMap` porque el orden del listado tiene que
    /// ser estable: un panel que cambia el orden entre dos peticiones idénticas
    /// es un panel que parece roto.
    unidades: Mutex<BTreeMap<String, UnitStatus>>,
    /// Cuántas recargas se le han pedido.
    ///
    /// Un contador y no un `bool`: lo que las pruebas miden es que la recarga
    /// **ocurre** —una por creación—, y con un booleano una recarga de más
    /// pasaría desapercibida.
    recargas: AtomicU32,
    /// El canal de avisos de cambio ([`UnitWatcher`]).
    ///
    /// El doble no mira systemd, así que no hay ninguna señal que reenviar: los
    /// avisos los emite la prueba con [`FakeSystemd::notificar`]. Es exactamente
    /// lo que hace falta para medir el fan-out y el stream sin un bus de sesión.
    avisos: broadcast::Sender<()>,
    /// El canal del ciclo de vida de los jobs ([`ProgresoDeTrabajos`]).
    ///
    /// Igual que el de avisos: sin bus no hay señales que reenviar, así que los
    /// eventos los emite la prueba con [`FakeSystemd::emitir`]. Es lo que permite
    /// medir el canal de trabajos —y su endpoint— sin un systemd de verdad.
    trabajos: broadcast::Sender<ProgresoDeJob>,
}

impl Default for FakeSystemd {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeSystemd {
    /// Un doble que no conoce ninguna unidad.
    pub fn new() -> Self {
        let (avisos, _) = broadcast::channel(CAPACIDAD_DE_AVISOS);
        let (trabajos, _) = broadcast::channel(CAPACIDAD_DE_TRABAJOS);

        Self {
            unidades: Mutex::new(BTreeMap::new()),
            recargas: AtomicU32::new(0),
            avisos,
            trabajos,
        }
    }

    /// Un doble con las unidades que se le den.
    pub fn con(unidades: impl IntoIterator<Item = UnitStatus>) -> Self {
        let doble = Self::new();

        {
            let mut conocidas = doble.bloquear();
            for unidad in unidades {
                conocidas.insert(unidad.name.as_str().to_owned(), unidad);
            }
        }

        doble
    }

    /// Un doble con dos quadlets de muestra, para mirar el panel sin systemd.
    ///
    /// Las dos llevan fichero de origen de quadlet, que es lo que las hace pasar
    /// por [`UnitStatus::is_quadlet`]: sin él no serían quadlets y el doble de
    /// muestra saldría vacío.
    pub fn demo() -> Self {
        Self::con([
            UnitStatus {
                name: UnitName::sin_validar("podpot-demo-db.service"),
                active_state: ActiveState::Active,
                sub_state: "running".to_owned(),
                unit_file_state: UnitFileState::Generated,
                origin: UnitOrigin::desde(
                    "/home/lorenzo/.config/containers/systemd/podpot-demo-db.container",
                ),
            },
            UnitStatus {
                name: UnitName::sin_validar("podpot-demo-web.service"),
                active_state: ActiveState::Inactive,
                sub_state: "dead".to_owned(),
                unit_file_state: UnitFileState::Generated,
                origin: UnitOrigin::desde(
                    "/home/lorenzo/.config/containers/systemd/podpot-demo-web.container",
                ),
            },
        ])
    }

    /// Cuántas recargas se le han pedido.
    pub fn recargas(&self) -> u32 {
        self.recargas.load(Ordering::SeqCst)
    }

    /// Emite un aviso de cambio, como si systemd hubiera señalado algo.
    ///
    /// Es la mitad que el doble añade al puerto [`UnitWatcher`]: sin bus no hay
    /// señales que reenviar, así que quien emite el aviso a mano es la prueba que
    /// está midiendo el fan-out o el stream. No hay error posible: si nadie está
    /// suscrito el aviso se pierde, que es justo lo que hace un canal sin
    /// receptores.
    pub fn notificar(&self) {
        let _ = self.avisos.send(());
    }

    /// Emite un evento del ciclo de vida de un job, como si systemd lo hubiera
    /// señalado.
    ///
    /// Es la mitad que el doble añade al puerto [`ProgresoDeTrabajos`]: sin bus no
    /// hay señales de `JobNew`/`JobRemoved` que reenviar, así que quien emite el
    /// evento a mano es la prueba que está midiendo el canal o su endpoint. No hay
    /// error posible: si nadie está suscrito el evento se pierde, que es justo lo
    /// que hace un canal sin receptores.
    pub fn emitir(&self, progreso: ProgresoDeJob) {
        let _ = self.trabajos.send(progreso);
    }

    /// Toma el candado de las unidades sin poder entrar en pánico.
    ///
    /// Si un hilo entró en pánico con el candado tomado, el mapa sigue siendo
    /// coherente —las operaciones son de una línea—, así que se sigue con él en
    /// vez de propagar el pánico: un doble de pruebas que se cae porque otra
    /// prueba se cayó no ayuda a diagnosticar nada.
    fn bloquear(&self) -> MutexGuard<'_, BTreeMap<String, UnitStatus>> {
        self.unidades
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner())
    }
}

#[async_trait]
impl SystemdClient for FakeSystemd {
    async fn quadlets(&self) -> Result<Vec<UnitStatus>, SystemdError> {
        let conocidas = self.bloquear();

        Ok(conocidas
            .values()
            .filter(|unidad| unidad.is_quadlet())
            .cloned()
            .collect())
    }

    async fn status(&self, unit: &UnitName) -> Result<UnitStatus, SystemdError> {
        let conocidas = self.bloquear();

        conocidas
            .get(unit.as_str())
            .cloned()
            .ok_or_else(|| SystemdError::UnitNotFound {
                unit: unit.as_str().to_owned(),
            })
    }

    async fn action(
        &self,
        unit: &UnitName,
        action: UnitAction,
    ) -> Result<UnitStatus, SystemdError> {
        let mut conocidas = self.bloquear();

        let estado =
            conocidas
                .get_mut(unit.as_str())
                .ok_or_else(|| SystemdError::UnitNotFound {
                    unit: unit.as_str().to_owned(),
                })?;

        // Los dos desenlaces que importan: parar deja la unidad parada y las
        // otras dos la dejan en marcha. Arrancar lo que ya está en marcha no es
        // un error —systemd tampoco lo trata como tal—, así que el resultado es
        // el mismo.
        match action {
            UnitAction::Stop => {
                estado.active_state = ActiveState::Inactive;
                estado.sub_state = "dead".to_owned();
            }
            UnitAction::Start | UnitAction::Restart => {
                estado.active_state = ActiveState::Active;
                estado.sub_state = "running".to_owned();
            }
        }

        Ok(estado.clone())
    }

    async fn reload(&self) -> Result<(), SystemdError> {
        // No hay nada que recargar —el doble no lee ficheros—, pero la llamada
        // se apunta: lo que las pruebas miden es que la creación la haga.
        self.recargas.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }
}

impl UnitWatcher for FakeSystemd {
    /// Se suscribe a los avisos que emita [`FakeSystemd::notificar`].
    ///
    /// Es el mismo contrato que cumple el cliente real por señales de D-Bus, pero
    /// sin bus: lo que se mide con él —el fan-out, el stream— es lo mismo, porque
    /// los dos sólo hablan de «algo cambió».
    fn subscribe(&self) -> broadcast::Receiver<()> {
        self.avisos.subscribe()
    }
}

impl ProgresoDeTrabajos for FakeSystemd {
    /// Se suscribe a los eventos que emita [`FakeSystemd::emitir`].
    ///
    /// Es el mismo contrato que cumple el cliente real por señales de D-Bus, pero
    /// sin bus: lo que se mide con él —el canal de trabajos y su endpoint— es lo
    /// mismo, porque los dos sólo hablan de transiciones de job.
    fn observar(&self) -> broadcast::Receiver<ProgresoDeJob> {
        self.trabajos.subscribe()
    }
}
