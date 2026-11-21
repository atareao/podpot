import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { App as AppDeAntd } from 'antd'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { unidad } from '../../tests/fixtures'
import { ErrorDeApi } from '../api/envelope'
import { accionDeUnidad, listarUnidades } from '../api/units'
import { borrarQuadlet, crearQuadlet, editarQuadlet, obtenerQuadlet } from '../api/quadlets'
import type { QuadletAfectada, QuadletDetalle } from '../types/api'
import Unidades from './unidades'

// La capa de red se dobla entera: las pruebas de pantalla no hablan con la API.
vi.mock('../api/units', () => ({
  listarUnidades: vi.fn(),
  accionDeUnidad: vi.fn(),
}))

vi.mock('../api/quadlets', () => ({
  crearQuadlet: vi.fn(),
  editarQuadlet: vi.fn(),
  borrarQuadlet: vi.fn(),
  obtenerQuadlet: vi.fn(),
}))

function montar() {
  const cliente = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  })
  return render(
    <QueryClientProvider client={cliente}>
      <AppDeAntd>
        <Unidades />
      </AppDeAntd>
    </QueryClientProvider>,
  )
}

/** Promesa que el test resuelve a mano, para observar el estado de carga. */
function diferido<T>() {
  let resolver!: (valor: T) => void
  const promesa = new Promise<T>((resolverla) => {
    resolver = resolverla
  })
  return { promesa, resolver: (valor: T) => resolver(valor) }
}

beforeEach(() => {
  vi.resetAllMocks()
})

describe('pantalla de quadlets', () => {
  it('lista cada quadlet con su estado', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', active_state: 'active', sub_state: 'running' }),
      unidad({ name: 'db.service', active_state: 'failed', sub_state: 'failed' }),
    ])

    montar()

    expect(await screen.findByText('web.service')).toBeInTheDocument()
    expect(screen.getByText('db.service')).toBeInTheDocument()
    expect(screen.getByText('En ejecución')).toBeInTheDocument()
    expect(screen.getByText('Fallida')).toBeInTheDocument()
  })

  it('muestra un aviso cuando no hay quadlets', async () => {
    vi.mocked(listarUnidades).mockResolvedValue([])

    montar()

    expect(await screen.findByText('No hay quadlets')).toBeInTheDocument()
  })

  it('ante un error de carga lo explica y permite reintentar', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockRejectedValueOnce(new ErrorDeApi(500, 'internal_error', 'no se pudo hablar con systemd'))
      .mockResolvedValueOnce([unidad({ name: 'web.service' })])

    montar()

    expect(await screen.findByText('No se pudieron cargar las quadlets')).toBeInTheDocument()
    expect(screen.getByText('no se pudo hablar con systemd')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Reintentar' }))

    expect(await screen.findByText('web.service')).toBeInTheDocument()
  })

  it('arranca una quadlet, avisa del resultado y refresca el estado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([
        unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
      ])
      .mockResolvedValueOnce([
        unidad({ name: 'web.service', active_state: 'active', sub_state: 'running' }),
      ])
    vi.mocked(accionDeUnidad).mockResolvedValue({ name: 'web.service', active_state: 'active' })

    montar()

    expect(await screen.findByText('Parada')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.service' }))

    expect(await screen.findByText(/«web.service» arrancada/i)).toBeInTheDocument()
    expect(vi.mocked(accionDeUnidad)).toHaveBeenCalledWith('web.service', 'start')
    // El refresco vuelve a pedir la lista y termina mostrando el estado nuevo.
    expect(await screen.findByText('En ejecución')).toBeInTheDocument()
    await waitFor(() => expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(2))
  })

  it('un error de acción avisa y mantiene el estado real de la unidad', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', active_state: 'inactive', sub_state: 'dead' }),
    ])
    vi.mocked(accionDeUnidad).mockRejectedValue(
      new ErrorDeApi(409, 'unit_conflict', 'la unidad está en transición'),
    )

    montar()

    expect(await screen.findByText('Parada')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Arrancar web.service' }))

    expect(await screen.findByText('la unidad está en transición')).toBeInTheDocument()
    // Sin refresco: la tabla sigue enseñando el estado real de la unidad.
    expect(screen.getByText('Parada')).toBeInTheDocument()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })

  it('crea una quadlet desde el formulario, avisa y refresca el listado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([unidad({ name: 'web.service' })])
    vi.mocked(crearQuadlet).mockResolvedValue({
      file_name: 'web.container',
      unit_name: 'web.service',
      kind: 'container',
    })

    montar()

    expect(await screen.findByText('No hay quadlets')).toBeInTheDocument()

    await usuario.type(screen.getByLabelText('Nombre'), 'web')
    await usuario.type(screen.getByLabelText('Imagen'), 'docker.io/library/nginx')
    await usuario.click(screen.getByRole('button', { name: 'Crear' }))

    expect(await screen.findByText(/«web.service» creada/i)).toBeInTheDocument()
    expect(vi.mocked(crearQuadlet)).toHaveBeenCalledWith({
      name: 'web',
      kind: 'container',
      image: 'docker.io/library/nginx',
    })
    // El refresco vuelve a pedir la lista y termina mostrando la quadlet nueva.
    expect(await screen.findByText('web.service')).toBeInTheDocument()
    await waitFor(() => expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(2))
  })

  it('un error de creación (colisión) muestra el mensaje y no refresca el listado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([])
    vi.mocked(crearQuadlet).mockRejectedValue(
      new ErrorDeApi(409, 'duplicate', 'ya existe una quadlet «web»'),
    )

    montar()

    expect(await screen.findByText('No hay quadlets')).toBeInTheDocument()

    await usuario.type(screen.getByLabelText('Nombre'), 'web')
    await usuario.type(screen.getByLabelText('Imagen'), 'nginx')
    await usuario.click(screen.getByRole('button', { name: 'Crear' }))

    expect(await screen.findByText('ya existe una quadlet «web»')).toBeInTheDocument()
    // Sin refresco como si hubiera tenido éxito.
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })
})

describe('edición de quadlets desde el panel', () => {
  const contenedor: QuadletDetalle = {
    name: 'web',
    kind: 'container',
    image: 'docker.io/library/nginx',
  }

  it('al pulsar «Editar» pide la definición y precarga el tipo y los campos reales', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue(contenedor)

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))

    // La definición se pide por nombre de fichero, no por nombre de unidad.
    await waitFor(() => expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith('web.container', expect.anything()))
    // El tipo y los campos salen del GET: nombre e imagen precargados.
    expect(screen.getByLabelText('Nombre')).toHaveValue('web')
    expect(screen.getByLabelText('Imagen')).toHaveValue('docker.io/library/nginx')
  })

  it('precarga el tipo que devuelve la definición', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({
        name: 'web-data-volume.service',
        file_name: 'web-data.volume',
        unit_file_state: 'generated',
      }),
    ])
    // Un volumen no pide imagen: si el formulario la pidiera, no vendría del GET.
    vi.mocked(obtenerQuadlet).mockResolvedValue({ name: 'web-data', kind: 'volume' })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web-data-volume.service' }))

    // El tipo sale de la definición leída y el fichero, de la API.
    await waitFor(() =>
      expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith('web-data.volume', expect.anything()),
    )
    expect(await screen.findByLabelText('Nombre')).toHaveValue('web-data')
    expect(screen.queryByLabelText('Imagen')).not.toBeInTheDocument()
    expect(screen.getByLabelText('Tipo de quadlet')).toBeDisabled()
  })

  it('mientras llega la definición avisa de que está cargando y aún no abre el formulario', async () => {
    const usuario = userEvent.setup()
    const definicion = diferido<QuadletDetalle>()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockReturnValue(definicion.promesa)

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))

    expect(await screen.findByText(/cargando la quadlet/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Guardar' })).not.toBeInTheDocument()

    definicion.resolver(contenedor)

    expect(await screen.findByRole('button', { name: 'Guardar' })).toBeInTheDocument()
  })

  it('si la definición no se puede leer, muestra el error de la API y no abre el formulario', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockRejectedValue(
      new ErrorDeApi(404, 'not_found', 'no existe la quadlet «web.container»'),
    )

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))

    expect(await screen.findByText('no existe la quadlet «web.container»')).toBeInTheDocument()
    // Sin formulario con valores inventados: ni Guardar ni campos.
    expect(screen.queryByRole('button', { name: 'Guardar' })).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Nombre')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Imagen')).not.toBeInTheDocument()
  })

  it('guarda con PATCH sobre el nombre de fichero y refresca el listado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([unidad({ name: 'web.service', unit_file_state: 'generated' })])
      .mockResolvedValueOnce([unidad({ name: 'web.service' })])
    vi.mocked(obtenerQuadlet).mockResolvedValue(contenedor)
    vi.mocked(editarQuadlet).mockResolvedValue({
      file_name: 'web.container',
      unit_name: 'web.service',
      kind: 'container',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Guardar' }))

    expect(await screen.findByText(/«web.service» editada/i)).toBeInTheDocument()
    // El identificador de la ruta es el nombre de fichero, no el de unidad.
    expect(vi.mocked(editarQuadlet)).toHaveBeenCalledWith('web.container', {
      name: 'web',
      kind: 'container',
      image: 'docker.io/library/nginx',
    })
    await waitFor(() => expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(2))
  })

  it('conserva los campos que el formulario no expone y devolvió la definición', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue({
      ...contenedor,
      publish_ports: ['8080:80'],
      volumes: ['datos:/var/lib/datos'],
      environments: ['TZ=Europe/Madrid'],
      container_name: 'web-prod',
    })
    vi.mocked(editarQuadlet).mockResolvedValue({
      file_name: 'web.container',
      unit_name: 'web.service',
      kind: 'container',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Guardar' }))

    // El PATCH reemplaza el struct entero: lo que el formulario no muestra se
    // devuelve tal cual para no borrarlo del fichero.
    await waitFor(() =>
      expect(vi.mocked(editarQuadlet)).toHaveBeenCalledWith('web.container', {
        name: 'web',
        kind: 'container',
        image: 'docker.io/library/nginx',
        container_name: 'web-prod',
        publish_ports: ['8080:80'],
        volumes: ['datos:/var/lib/datos'],
        environments: ['TZ=Europe/Madrid'],
      }),
    )
  })

  it('un error de edición muestra el mensaje de la API y no refresca el listado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue(contenedor)
    vi.mocked(editarQuadlet).mockRejectedValue(
      new ErrorDeApi(400, 'invalid_value', 'la imagen no admite saltos de línea'),
    )

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Guardar' }))

    expect(await screen.findByText('la imagen no admite saltos de línea')).toBeInTheDocument()
    // Sin refresco: sigue en modo edición para poder corregir.
    expect(screen.getByRole('button', { name: 'Guardar' })).toBeInTheDocument()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })

  it('cancelar la edición vuelve al formulario de creación sin llamar a la API', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue(contenedor)

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Cancelar' }))

    expect(await screen.findByRole('button', { name: 'Crear' })).toBeInTheDocument()
    expect(vi.mocked(editarQuadlet)).not.toHaveBeenCalled()
  })

  it('deshabilita las acciones de fila mientras se lee la definición y mientras se guarda', async () => {
    const usuario = userEvent.setup()
    const definicion = diferido<QuadletDetalle>()
    const guardado = diferido<QuadletAfectada>()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({
        name: 'web.service',
        active_state: 'inactive',
        sub_state: 'dead',
        unit_file_state: 'generated',
      }),
    ])
    vi.mocked(obtenerQuadlet).mockReturnValue(definicion.promesa)
    vi.mocked(editarQuadlet).mockReturnValue(guardado.promesa)

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))

    // Con una lectura en vuelo no se encadenan operaciones sobre esa fila.
    expect(screen.getByRole('button', { name: 'Arrancar web.service' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Editar web.service' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Borrar web.service' })).toBeDisabled()

    definicion.resolver(contenedor)
    await usuario.click(await screen.findByRole('button', { name: 'Guardar' }))

    // Y con un guardado en vuelo, tampoco.
    expect(screen.getByRole('button', { name: 'Borrar web.service' })).toBeDisabled()

    guardado.resolver({ file_name: 'web.container', unit_name: 'web.service', kind: 'container' })
    expect(await screen.findByText(/«web.service» editada/i)).toBeInTheDocument()
  })
})

describe('identidad publicada por la API', () => {
  it('edita usando el file_name publicado, sin invertir el nombre de unidad', async () => {
    const usuario = userEvent.setup()
    // `web-volume.service` no permite reconstruir el fichero: la inversa
    // resolvería al volumen `web.volume`, pero el fichero real es otro.
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.container',
        unit_file_state: 'generated',
      }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue({
      name: 'web-volume',
      kind: 'container',
      image: 'nginx',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web-volume.service' }))

    // La definición se lee por el fichero que publicó la API.
    await waitFor(() =>
      expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith(
        'web-volume.container',
        expect.anything(),
      ),
    )
  })

  it('borra usando el file_name publicado, sin invertir el nombre de unidad', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([
        unidad({
          name: 'web-volume.service',
          file_name: 'web-volume.container',
          unit_file_state: 'generated',
        }),
      ])
      .mockResolvedValueOnce([])
    vi.mocked(obtenerQuadlet).mockResolvedValue({
      name: 'web-volume',
      kind: 'container',
      image: 'nginx',
    })
    vi.mocked(borrarQuadlet).mockResolvedValue({
      file_name: 'web-volume.container',
      unit_name: 'web-volume.service',
      kind: 'container',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Borrar web-volume.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Sí, borrar' }))

    expect(await screen.findByText(/«web-volume.service» borrada/i)).toBeInTheDocument()
    expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith(
      'web-volume.container',
      expect.anything(),
    )
    expect(vi.mocked(borrarQuadlet)).toHaveBeenCalledWith('web-volume.container')
  })
})

describe('dos quadlets con el mismo nombre de unidad', () => {
  it('edita el file_name correcto de cada una', async () => {
    const usuario = userEvent.setup()
    // Un contenedor `web-volume` y un volumen `web` comparten la unidad
    // `web-volume.service`; el panel debe operar sobre el fichero de cada fila.
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.container',
        unit_file_state: 'generated',
      }),
      unidad({ name: 'web-volume.service', file_name: 'web.volume', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue({ name: 'x', kind: 'container', image: 'nginx' })

    montar()

    const botonesEditar = await screen.findAllByRole('button', {
      name: 'Editar web-volume.service',
    })
    expect(botonesEditar).toHaveLength(2)

    await usuario.click(botonesEditar[0]!)
    await waitFor(() =>
      expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith(
        'web-volume.container',
        expect.anything(),
      ),
    )

    // Se cierra la edición y se abre la de la otra fila: cada una por su fichero.
    await usuario.click(await screen.findByRole('button', { name: 'Cancelar' }))
    await usuario.click(botonesEditar[1]!)
    await waitFor(() =>
      expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith('web.volume', expect.anything()),
    )
  })
})

describe('borrado de quadlets desde el panel', () => {
  it('pide confirmación antes de borrar y no envía petición si se cancela', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Borrar web.service' }))

    expect(await screen.findByText('¿Borrar la quadlet «web.service»?')).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Cancelar' }))

    // Ni verificación ni borrado: cancelar no sale de la fila.
    expect(vi.mocked(obtenerQuadlet)).not.toHaveBeenCalled()
    expect(vi.mocked(borrarQuadlet)).not.toHaveBeenCalled()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })

  it('verifica la identidad con un GET antes de enviar el DELETE, y luego borra', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([unidad({ name: 'web.service', unit_file_state: 'generated' })])
      .mockResolvedValueOnce([])
    vi.mocked(obtenerQuadlet).mockResolvedValue({ name: 'web', kind: 'container' })
    vi.mocked(borrarQuadlet).mockResolvedValue({
      file_name: 'web.container',
      unit_name: 'web.service',
      kind: 'container',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Borrar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Sí, borrar' }))

    expect(await screen.findByText(/«web.service» borrada/i)).toBeInTheDocument()
    expect(vi.mocked(obtenerQuadlet)).toHaveBeenCalledWith('web.container', expect.anything())
    expect(vi.mocked(borrarQuadlet)).toHaveBeenCalledWith('web.container')
    // Primero la verificación, después el borrado: nunca al revés.
    expect(vi.mocked(obtenerQuadlet).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(borrarQuadlet).mock.invocationCallOrder[0] ?? 0,
    )
  })

  it('si la verificación falla no envía el DELETE y muestra el error', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({
        name: 'web-volume.service',
        file_name: 'web-volume.container',
        unit_file_state: 'generated',
      }),
    ])
    // El fichero publicado no existe (se borró fuera del panel): el DELETE no sale.
    vi.mocked(obtenerQuadlet).mockRejectedValue(
      new ErrorDeApi(404, 'not_found', 'no existe la quadlet «web-volume.container»'),
    )

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Borrar web-volume.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Sí, borrar' }))

    expect(
      await screen.findByText('no existe la quadlet «web-volume.container»'),
    ).toBeInTheDocument()
    expect(vi.mocked(borrarQuadlet)).not.toHaveBeenCalled()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })

  it('borra al confirmar y refresca el listado sin la quadlet', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([unidad({ name: 'web.service', unit_file_state: 'generated' })])
      .mockResolvedValueOnce([])
    vi.mocked(obtenerQuadlet).mockResolvedValue({ name: 'web', kind: 'container' })
    vi.mocked(borrarQuadlet).mockResolvedValue({
      file_name: 'web.container',
      unit_name: 'web.service',
      kind: 'container',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Borrar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Sí, borrar' }))

    expect(await screen.findByText(/«web.service» borrada/i)).toBeInTheDocument()
    expect(vi.mocked(borrarQuadlet)).toHaveBeenCalledWith('web.container')
    await waitFor(() => expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(2))
    expect(await screen.findByText('No hay quadlets')).toBeInTheDocument()
  })

  it('borrar la quadlet que se está editando cierra el formulario', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades)
      .mockResolvedValueOnce([unidad({ name: 'web.service', unit_file_state: 'generated' })])
      .mockResolvedValueOnce([])
    vi.mocked(obtenerQuadlet).mockResolvedValue({ name: 'web', kind: 'container', image: 'nginx' })
    vi.mocked(borrarQuadlet).mockResolvedValue({
      file_name: 'web.container',
      unit_name: 'web.service',
      kind: 'container',
    })

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Editar web.service' }))
    expect(await screen.findByRole('button', { name: 'Guardar' })).toBeInTheDocument()

    await usuario.click(screen.getByRole('button', { name: 'Borrar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Sí, borrar' }))

    // El formulario que editaba esa quadlet ya no tiene nada que editar.
    expect(await screen.findByRole('button', { name: 'Crear' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Guardar' })).not.toBeInTheDocument()
  })

  it('un error de borrado muestra el mensaje de la API y no refresca el listado', async () => {
    const usuario = userEvent.setup()
    vi.mocked(listarUnidades).mockResolvedValue([
      unidad({ name: 'web.service', unit_file_state: 'generated' }),
    ])
    vi.mocked(obtenerQuadlet).mockResolvedValue({ name: 'web', kind: 'container' })
    vi.mocked(borrarQuadlet).mockRejectedValue(
      new ErrorDeApi(404, 'not_found', 'no existe la quadlet «web.container»'),
    )

    montar()

    await usuario.click(await screen.findByRole('button', { name: 'Borrar web.service' }))
    await usuario.click(await screen.findByRole('button', { name: 'Sí, borrar' }))

    expect(await screen.findByText('no existe la quadlet «web.container»')).toBeInTheDocument()
    expect(vi.mocked(listarUnidades)).toHaveBeenCalledTimes(1)
  })
})
