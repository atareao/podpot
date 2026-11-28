/**
 * Contrato que fijan estos tests para GREEN:
 * - `ModalDeDetalles({ abierto, unidad, detalle, cargando?, error?, onCerrar })`.
 * - Solo se muestran campos reales de `GET /api/v1/quadlets/{name}`: imagen,
 *   `container_name`, puertos, volúmenes, entornos, red/subred y pod.
 * - Con `error`, el modal sigue siendo cerrable y el panel de detrás sigue vivo.
 */

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { unidad } from '../../../tests/fixtures'
import { ModalDeDetalles } from './ModalDeDetalles'
import type { QuadletDetalle } from '../../types/api'

const detalle: QuadletDetalle = {
  name: 'web',
  kind: 'container',
  image: 'docker.io/library/nginx:1.27',
  container_name: 'web-container',
  publish_ports: ['8080:80', '8443:443'],
  volumes: ['datos:/var/lib/web'],
  environments: ['MODO=produccion'],
  network_name: 'web-red',
  subnet: '10.8.0.0/24',
  pod_name: 'web-pod',
}

function montar(conError = false) {
  const onCerrar = vi.fn()
  render(
    <>
      <button type="button">Panel de quadlets</button>
      <ModalDeDetalles
        abierto
        unidad={unidad({ name: 'web.service', file_name: 'web.container' })}
        detalle={conError ? null : detalle}
        error={conError ? 'No se pudo leer la definición de la quadlet' : null}
        onCerrar={onCerrar}
      />
    </>,
  )
  return { onCerrar }
}

describe('ModalDeDetalles', () => {
  it('muestra únicamente los campos que devuelve la API de detalle', () => {
    montar()
    const dialog = screen.getByRole('dialog')

    for (const valor of [
      'docker.io/library/nginx:1.27',
      'web-container',
      '8080:80',
      '8443:443',
      'datos:/var/lib/web',
      'MODO=produccion',
      'web-red',
      '10.8.0.0/24',
      'web-pod',
    ]) {
      expect(within(dialog).getByText(valor)).toBeInTheDocument()
    }

    expect(within(dialog).getByText(/imagen/i)).toBeInTheDocument()
    expect(within(dialog).getByText(/red/i)).toBeInTheDocument()
    expect(within(dialog).getByText(/pod/i)).toBeInTheDocument()
  })

  it('usa el token de texto secundario con fallback accesible en las etiquetas de campo', () => {
    montar()

    const etiqueta = screen.getByText('Imagen')
    expect(etiqueta.getAttribute('style')).toContain('var(--ant-color-text-secondary, #595959)')
    expect(etiqueta.getAttribute('style')).not.toContain('#8c8c8c')
  })

  it('no presenta datos que el endpoint no expone', () => {
    montar()

    expect(
      screen.queryByText(/health|healthcheck|estado de salud|labels|etiquetas|actualizaciones|updates/i),
    ).not.toBeInTheDocument()
  })

  it('muestra el error de carga y permite cerrar sin romper el panel', async () => {
    const usuario = userEvent.setup()
    const { onCerrar } = montar(true)

    expect(screen.getByText('No se pudo leer la definición de la quadlet')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Panel de quadlets' })).toBeInTheDocument()

    const botonesDeCierre = screen.getAllByRole('button', { name: /cerrar|close/i })
    await usuario.click(botonesDeCierre[0]!)

    expect(onCerrar).toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Panel de quadlets' })).toBeInTheDocument()
  })
})
