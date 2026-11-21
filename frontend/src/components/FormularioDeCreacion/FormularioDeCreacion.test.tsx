import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { FormularioDeCreacion } from './FormularioDeCreacion'

/**
 * Abre el desplegable del `Select` y elige una opción por su etiqueta visible.
 *
 * No se usa `getByRole('option')`: antd mantiene una lista accesible oculta
 * (`role="option"` con `aria-label`) además de las opciones visibles, y pulsar
 * la oculta no selecciona nada.
 */
async function elegirTipo(usuario: ReturnType<typeof userEvent.setup>, etiqueta: string) {
  await usuario.click(screen.getByLabelText('Tipo de quadlet'))
  await usuario.click(await screen.findByText(etiqueta))
}

describe('formulario de creación de quadlets', () => {
  it('muestra los campos mínimos del tipo elegido', async () => {
    const usuario = userEvent.setup()
    render(<FormularioDeCreacion onCrear={vi.fn()} />)

    // Arranca en `.container`: su campo mínimo es la imagen.
    expect(screen.getByLabelText('Imagen')).toBeInTheDocument()
    expect(screen.getByLabelText('Nombre')).toBeInTheDocument()

    // Un volumen no tiene campos mínimos propios: la imagen desaparece.
    await elegirTipo(usuario, 'Volumen (.volume)')

    expect(screen.queryByLabelText('Imagen')).not.toBeInTheDocument()
    expect(screen.getByLabelText('Nombre')).toBeInTheDocument()
  })

  it('envía el tipo, el nombre y los campos mínimos al confirmar', async () => {
    const usuario = userEvent.setup()
    const alCrear = vi.fn().mockResolvedValue(true)
    render(<FormularioDeCreacion onCrear={alCrear} />)

    await usuario.type(screen.getByLabelText('Nombre'), 'web')
    await usuario.type(screen.getByLabelText('Imagen'), 'docker.io/library/nginx')
    await usuario.click(screen.getByRole('button', { name: 'Crear' }))

    await waitFor(() =>
      expect(alCrear).toHaveBeenCalledWith({
        name: 'web',
        kind: 'container',
        image: 'docker.io/library/nginx',
      }),
    )
  })

  it('no llama a la API si faltan campos obligatorios', async () => {
    const usuario = userEvent.setup()
    const alCrear = vi.fn()
    render(<FormularioDeCreacion onCrear={alCrear} />)

    await usuario.click(screen.getByRole('button', { name: 'Crear' }))

    expect(await screen.findByText('Introduce un nombre')).toBeInTheDocument()
    expect(alCrear).not.toHaveBeenCalled()
  })

  it('limpia el formulario solo cuando la creación termina bien', async () => {
    const usuario = userEvent.setup()
    const alCrear = vi.fn().mockResolvedValue(false)
    render(<FormularioDeCreacion onCrear={alCrear} />)

    await usuario.type(screen.getByLabelText('Nombre'), 'web')
    await usuario.type(screen.getByLabelText('Imagen'), 'nginx')
    await usuario.click(screen.getByRole('button', { name: 'Crear' }))

    // Al fallar (devuelve `false`), lo escrito se conserva para reintentar.
    await waitFor(() => expect(alCrear).toHaveBeenCalledTimes(1))
    expect(screen.getByLabelText('Nombre')).toHaveValue('web')
  })

  it('en modo edición precarga el tipo y el nombre, y no deja tocarlos', async () => {
    const usuario = userEvent.setup()
    const alCrear = vi.fn().mockResolvedValue(true)
    render(
      <FormularioDeCreacion
        onCrear={alCrear}
        valoresIniciales={{ kind: 'volume', name: 'web-data' }}
        etiquetaDeEnvio="Guardar"
        identidadFija
        alCancelar={vi.fn()}
      />,
    )

    // El volumen precargado no pide imagen; el nombre sale del stem, fijo.
    expect(screen.getByLabelText('Nombre')).toHaveValue('web-data')
    expect(screen.queryByLabelText('Imagen')).not.toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Guardar' }))

    await waitFor(() =>
      expect(alCrear).toHaveBeenCalledWith({ name: 'web-data', kind: 'volume' }),
    )
  })

  it('no pinta el botón de cancelar si no hay acción de cancelar', () => {
    render(<FormularioDeCreacion onCrear={vi.fn()} />)

    expect(screen.queryByRole('button', { name: 'Cancelar' })).not.toBeInTheDocument()
  })

  it('avisa al cancelar sin enviar nada', async () => {
    const usuario = userEvent.setup()
    const alCrear = vi.fn()
    const alCancelar = vi.fn()
    render(<FormularioDeCreacion onCrear={alCrear} alCancelar={alCancelar} />)

    await usuario.click(screen.getByRole('button', { name: 'Cancelar' }))

    expect(alCancelar).toHaveBeenCalledTimes(1)
    expect(alCrear).not.toHaveBeenCalled()
  })
})
