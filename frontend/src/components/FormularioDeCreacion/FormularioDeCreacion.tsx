import { useState } from 'react'
import { Button } from 'antd'
import type { PeticionDeCreacion, TipoDeQuadlet } from '../../types/api'

const ETIQUETAS: Record<TipoDeQuadlet, string> = {
  container: 'Contenedor (.container)',
  volume: 'Volumen (.volume)',
  network: 'Red (.network)',
  pod: 'Pod (.pod)',
}

const OPCIONES = (Object.entries(ETIQUETAS) as Array<[TipoDeQuadlet, string]>).map(
  ([value, label]) => ({ value, label }),
)

export interface ValoresDeFormulario {
  kind: TipoDeQuadlet
  name: string
  image?: string
}

export interface FormularioDeCreacionProps {
  onCrear: (peticion: PeticionDeCreacion) => Promise<boolean> | boolean
  creando?: boolean
  valoresIniciales?: Partial<ValoresDeFormulario>
  etiquetaDeEnvio?: string
  identidadFija?: boolean
  alCancelar?: () => void
}

function construirPeticion(valores: ValoresDeFormulario): PeticionDeCreacion {
  const peticion: PeticionDeCreacion = { name: valores.name.trim(), kind: valores.kind }
  if (valores.kind === 'container') {
    peticion.image = valores.image?.trim()
  }
  return peticion
}

/**
 * Select nativo desplegable: mantiene accesible el desplegable con
 * `role="combobox"` y `role="option"` para que los tests puedan hacer
 * `getByLabelText` y `findByText(etiqueta)` exactamente como con Ant Design.
 * Usa un `<button>` como trigger para que `getByLabelText` devuelva un elemento
 * que pueda estar deshabilitado (`toBeDisabled()`).
 */
function SelectNativo({
  value,
  disabled,
  onChange,
}: {
  value: TipoDeQuadlet
  disabled?: boolean
  onChange: (kind: TipoDeQuadlet) => void
}) {
  const [abierto, setAbierto] = useState(false)
  const etiqueta = ETIQUETAS[value]

  return (
    <div style={{ position: 'relative' }}>
      <label htmlFor="select-tipo">Tipo de quadlet</label>
      <button
        id="select-tipo"
        type="button"
        role="combobox"
        aria-expanded={abierto}
        aria-label="Tipo de quadlet"
        onClick={() => !disabled && setAbierto(!abierto)}
        disabled={disabled}
        tabIndex={0}
        style={{
          width: 200,
          border: '1px solid #d9d9d9',
          borderRadius: 4,
          padding: '6px 11px',
          background: disabled ? '#fafafa' : '#fff',
          cursor: disabled ? 'not-allowed' : 'pointer',
          userSelect: 'none',
          textAlign: 'left',
        }}
      >
        {etiqueta}
      </button>
      {abierto && (
        <div
          role="listbox"
          style={{
            position: 'absolute',
            zIndex: 1050,
            width: 200,
            border: '1px solid #d9d9d9',
            borderTop: 'none',
            borderRadius: '0 0 4px 4px',
            background: '#fff',
            maxHeight: 256,
            overflowY: 'auto',
            boxShadow: '0 3px 6px -4px rgba(0,0,0,.12)',
          }}
        >
          {OPCIONES.map((opt) => (
            <div
              key={opt.value}
              role="option"
              aria-selected={opt.value === value}
              onClick={() => {
                if (!disabled) {
                  onChange(opt.value)
                  setAbierto(false)
                }
              }}
              style={{
                padding: '6px 11px',
                cursor: disabled ? 'not-allowed' : 'pointer',
                color: disabled ? 'rgba(0,0,0,.25)' : undefined,
                background: opt.value === value ? '#e6f7ff' : undefined,
              }}
            >
              {opt.label}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

/**
 * Formulario nativo para crear una quadlet nueva —y, con
 * `valoresIniciales`/`identidadFija`, para editar una existente—.
 *
 * Presentacional respecto a React Query: quien lo usa (`pages/unidades.tsx`)
 * conecta la mutación, avisa del resultado y refresca el listado. El selector de
 * tipo decide qué campos mínimos se piden: un `.container` necesita su imagen;
 * `volume`, `network` y `pod` se conforman con el nombre.
 */
export function FormularioDeCreacion({
  onCrear,
  creando = false,
  valoresIniciales,
  etiquetaDeEnvio = 'Crear',
  identidadFija = false,
  alCancelar,
}: FormularioDeCreacionProps) {
  const [kind, setKind] = useState(valoresIniciales?.kind ?? 'container')
  const [name, setName] = useState(valoresIniciales?.name ?? '')
  const [image, setImage] = useState(valoresIniciales?.image ?? '')
  const [errores, setErrores] = useState<Record<string, string>>({})

  const alEnviar = async (e: React.FormEvent) => {
    e.preventDefault()

    const nuevosErrores: Record<string, string> = {}
    if (!name.trim()) nuevosErrores.name = 'Introduce un nombre'
    if (kind === 'container' && !image.trim()) {
      nuevosErrores.image = 'Introduce una imagen'
    }
    setErrores(nuevosErrores)

    if (Object.keys(nuevosErrores).length > 0) return

    const guardada = await onCrear(construirPeticion({ kind, name, image }))
    if (guardada) {
      setKind(valoresIniciales?.kind ?? 'container')
      setName(valoresIniciales?.name ?? '')
      setImage('')
      setErrores({})
    }
  }

  return (
    <form onSubmit={alEnviar} className="antd-form">
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1rem',
          flexWrap: 'wrap',
        }}
      >
        <SelectNativo
          value={kind}
          disabled={identidadFija}
          onChange={setKind}
        />

        <div>
          <label htmlFor="input-nombre">Nombre</label>
          <input
            id="input-nombre"
            name="name"
            aria-label="Nombre"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="web"
            autoComplete="off"
            disabled={identidadFija}
            style={{
              width: 220,
              padding: '6px 11px',
              border: '1px solid #d9d9d9',
              borderRadius: 4,
              background: identidadFija ? '#fafafa' : '#fff',
            }}
          />
          {errores.name && <div style={{ color: '#ff4d4f', fontSize: '0.75rem', marginTop: 4 }}>{errores.name}</div>}
        </div>

        {kind === 'container' && (
          <div>
            <label htmlFor="input-imagen">Imagen</label>
            <input
              id="input-imagen"
              name="image"
              aria-label="Imagen"
              value={image}
              onChange={(e) => setImage(e.target.value)}
              placeholder="docker.io/library/nginx"
              autoComplete="off"
              style={{
                width: 320,
                padding: '6px 11px',
                border: '1px solid #d9d9d9',
                borderRadius: 4,
                background: '#fff',
              }}
            />
            {errores.image && <div style={{ color: '#ff4d4f', fontSize: '0.75rem', marginTop: 4 }}>{errores.image}</div>}
          </div>
        )}
      </div>

      <div style={{ marginTop: '0.5rem', display: 'flex', gap: '0.5rem' }}>
        <Button
          type="primary"
          htmlType="submit"
          className="antd-button-primary"
          loading={creando}
          disabled={identidadFija && kind !== valoresIniciales?.kind}
          style={{ width: 'auto' }}
        >
          {etiquetaDeEnvio}
        </Button>
        {alCancelar !== undefined && (
          <Button
            type="default"
            onClick={alCancelar}
            disabled={creando}
            className="antd-button-default"
          >
            Cancelar
          </Button>
        )}
      </div>
    </form>
  )
}