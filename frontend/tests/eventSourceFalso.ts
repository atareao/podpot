/**
 * Doble de `EventSource` para las pruebas.
 *
 * jsdom no trae `EventSource`, así que los tests del canal SSE necesitan un
 * sustituto. Este registra la URL, los oyentes por **tipo de evento** (el canal
 * usa un evento con nombre, `units`, no `onmessage`) y si se cerró; además deja
 * emitir eventos a mano para simular la instantánea que empuja el servidor.
 */
type Oyente = (evento: MessageEvent<string>) => void

export class EventSourceFalso {
  /** Instancias creadas, en orden: la última es la suscripción viva. */
  static instancias: EventSourceFalso[] = []

  readonly url: string
  cerrado = false
  /** El `message` por defecto (propiedad), por si el hook lo usara por error. */
  onmessage: Oyente | null = null

  private readonly oyentes = new Map<string, Set<Oyente>>()

  constructor(url: string) {
    this.url = url
    EventSourceFalso.instancias.push(this)
  }

  addEventListener(tipo: string, oyente: Oyente): void {
    const conjunto = this.oyentes.get(tipo) ?? new Set<Oyente>()
    conjunto.add(oyente)
    this.oyentes.set(tipo, conjunto)
  }

  removeEventListener(tipo: string, oyente: Oyente): void {
    this.oyentes.get(tipo)?.delete(oyente)
  }

  close(): void {
    this.cerrado = true
  }

  /**
   * Emite un evento, con el `data` serializado a JSON.
   *
   * Si `tipo` es `message`, también dispara el `onmessage` por defecto: así una
   * prueba puede comprobar que el hook **no** escucha el evento por defecto.
   */
  emitir(tipo: string, data: unknown): void {
    const evento = { data: JSON.stringify(data) } as MessageEvent<string>
    if (tipo === 'message') this.onmessage?.(evento)
    for (const oyente of this.oyentes.get(tipo) ?? []) oyente(evento)
  }

  static reset(): void {
    EventSourceFalso.instancias = []
  }
}
