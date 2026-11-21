//! Secretos que no se filtran al formatearse ni al registro.
//!
//! El secreto de firma de la sesión (`PODPOT_SECRET`) y el `client_secret` del
//! proveedor OIDC viajan envueltos en [`SecretString`]: un tipo que **redacta**
//! su contenido en `Debug` y en `Display`, lo expone únicamente por
//! [`SecretString::expose`] y borra la memoria que ocupaba al caer (`zeroize`).
//!
//! No es cosmético: el volcado de arranque (`Config::describe`) y cualquier
//! `?campo` en un `tracing!` imprimen el valor si el tipo no se defiende, y un
//! secreto que aparece en el registro deja de ser un secreto.

use std::fmt;

use zeroize::Zeroizing;

/// Marca que sustituye al valor en cualquier formato.
pub const REDACTED: &str = "<redactado>";

/// Cadena que guarda una credencial y no la deja salir por accidente.
#[derive(Clone)]
pub struct SecretString {
    /// El valor, puesto a cero al caer por `zeroize`.
    inner: Zeroizing<String>,
}

impl SecretString {
    /// Envuelve `value` para que no pueda filtrarse al formatearse.
    pub fn new(value: String) -> Self {
        Self {
            inner: Zeroizing::new(value),
        }
    }

    /// Devuelve el valor de la credencial.
    ///
    /// Es el único camino por el que el contenido sale del tipo: que sea un
    /// método con nombre es lo que hace visible cada sitio que lo usa.
    pub fn expose(&self) -> &str {
        self.inner.as_str()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretString").field(&REDACTED).finish()
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

#[cfg(test)]
mod tests {
    use super::{REDACTED, SecretString};

    /// El valor de una credencial no aparece ni en `Debug` ni en `Display`, y sí
    /// sale por el único camino que existe para leerlo.
    #[test]
    fn un_secreto_no_se_publica_al_formatearse() {
        const VALOR: &str = "credencial-distinguible-000000000000000000000000000000";
        let secreto = SecretString::new(VALOR.to_owned());

        for formateado in [format!("{secreto:?}"), format!("{secreto}")] {
            assert!(
                !formateado.contains(VALOR),
                "el secreto no puede aparecer formateado: {formateado}"
            );
            assert!(
                formateado.contains(REDACTED),
                "el secreto tiene que salir redactado: {formateado}"
            );
        }

        assert_eq!(
            secreto.expose(),
            VALOR,
            "el valor tiene que salir íntegro por `expose`"
        );
    }
}
