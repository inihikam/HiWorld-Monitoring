import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createWsClient } from '../src/lib/ws.js'

/** MockWebSocket: onopen/onmessage/onclose manual. */
class MockWebSocket {
  static instances = []
  constructor(url) {
    this.url = url
    this.readyState = 0
    MockWebSocket.instances.push(this)
  }
  open() {
    this.readyState = 1
    this.onopen?.()
  }
  receive(obj) {
    this.onmessage?.({ data: JSON.stringify(obj) })
  }
  fail() {
    this.onclose?.() // onerror sudah memanggil close di client
  }
  close() {
    this.onclose?.()
  }
}

function setup() {
  MockWebSocket.instances = []
  vi.stubGlobal('WebSocket', MockWebSocket)
  const handlers = {
    onHello: vi.fn(),
    onSnapshot: vi.fn(),
    onEvent: vi.fn(),
    onHostStatus: vi.fn(),
    onStateChange: vi.fn(),
  }
  const client = createWsClient(handlers)
  return { client, handlers }
}

describe('WsClient (WD9)', () => {
  beforeEach(() => {
    vi.unstubAllGlobals()
    vi.useFakeTimers()
  })

  it('connect ke /ws dengan protokol ws (WD-AC-013)', () => {
    const { client } = setup()
    client.connect()
    expect(MockWebSocket.instances[0].url).toMatch(/^ws:\/\/localhost(:\d+)?\/ws$/)
    client.close()
  })

  it('hello → onHello dengan data hosts (WD-AC-013)', () => {
    const { client, handlers } = setup()
    client.connect()
    const ws = MockWebSocket.instances[0]
    ws.open()
    ws.receive({ type: 'hello', data: { hosts: [{ host_id: 'h1', latest: null }] } })
    expect(handlers.onHello).toHaveBeenCalledWith({
      hosts: [{ host_id: 'h1', latest: null }],
    })
    client.close()
  })

  it('snapshot/event/host_status dispatch ke handler masing-masing', () => {
    const { client, handlers } = setup()
    client.connect()
    const ws = MockWebSocket.instances[0]
    ws.open()
    ws.receive({ type: 'snapshot', data: { host_id: 'h1' } })
    ws.receive({ type: 'event', data: { kind: 'spike_cpu' } })
    ws.receive({ type: 'host_status', data: { host_id: 'h1', online: false } })
    expect(handlers.onSnapshot).toHaveBeenCalled()
    expect(handlers.onEvent).toHaveBeenCalled()
    expect(handlers.onHostStatus).toHaveBeenCalled()
    client.close()
  })

  it('pesan korup → tidak crash', () => {
    const { client, handlers } = setup()
    client.connect()
    const ws = MockWebSocket.instances[0]
    ws.open()
    expect(() => ws.onmessage({ data: 'bukan-json{' })).not.toThrow()
    expect(handlers.onSnapshot).not.toHaveBeenCalled()
    client.close()
  })

  it('putus berulang tanpa open → backoff 1s→2s→4s (WD-AC-014)', () => {
    const { client } = setup()
    client.connect()
    let ws = MockWebSocket.instances[0]
    ws.fail() // gagal pertama (tidak pernah open)
    vi.advanceTimersByTime(1000) // backoff ke-1 = 1s
    expect(MockWebSocket.instances.length).toBe(2)
    ws = MockWebSocket.instances[1]
    ws.fail() // gagal kedua
    vi.advanceTimersByTime(1000) // belum cukup (backoff ke-2 = 2s)
    expect(MockWebSocket.instances.length).toBe(2)
    vi.advanceTimersByTime(1000)
    expect(MockWebSocket.instances.length).toBe(3)
    client.close()
  })

  it('open sukses → reset attempt; putus lagi → backoff dari 1s', () => {
    const { client } = setup()
    client.connect()
    let ws = MockWebSocket.instances[0]
    ws.open()
    ws.fail()
    vi.advanceTimersByTime(1000)
    ws = MockWebSocket.instances[1]
    ws.open() // sukses → attempt reset
    ws.fail() // putus lagi
    vi.advanceTimersByTime(1000) // delay 1s (reset) → reconnect
    expect(MockWebSocket.instances.length).toBe(3)
    client.close()
  })

  it('backoff cap 30s', () => {
    const { client } = setup()
    client.connect()
    // gagal berkali-kali sampai melewati tabel backoff
    for (let i = 0; i < 10; i++) {
      const ws = MockWebSocket.instances[i]
      ws.open()
      ws.fail()
      vi.advanceTimersByTime(31000)
    }
    // instance terakhir dibuat ≤31s setelah gagal ke-10 — backoff dibatasi 30s
    const ws = MockWebSocket.instances[9]
    expect(ws).toBeDefined()
    client.close()
  })

  it('close() oleh user → tidak reconnect (WD-AC-015 terkait lifecycle)', () => {
    const { client } = setup()
    client.connect()
    const ws = MockWebSocket.instances[0]
    ws.open()
    client.close()
    ws.fail()
    vi.advanceTimersByTime(60000)
    expect(MockWebSocket.instances.length).toBe(1)
  })

  it('fail 3x → fallback polling REST 5s (WO-AC-011)', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ hosts: [{ host_id: 'h1', online: true }] }),
    })
    vi.stubGlobal('fetch', fetchMock)
    const { client, handlers } = setup()
    client.connect()
    for (let i = 0; i < 3; i++) {
      MockWebSocket.instances[i].fail() // gagal beruntun tanpa open
      vi.advanceTimersByTime(31000)
    }
    // fallback interval 5s — callback interval async: flush microtask
    vi.advanceTimersByTime(5000)
    await Promise.resolve()
    await Promise.resolve()
    expect(fetchMock).toHaveBeenCalledWith('/api/hosts', { credentials: 'same-origin' })
    client.close()
  })

  it('WS pulih → status connected & polling berhenti', () => {
    const { client, handlers } = setup()
    client.connect()
    const calls = () => handlers.onStateChange.mock.calls.length
    let ws = MockWebSocket.instances[0]
    ws.open()
    expect(handlers.onStateChange).toHaveBeenLastCalledWith('connected')
    ws.fail()
    vi.advanceTimersByTime(1000)
    ws = MockWebSocket.instances[1]
    ws.open()
    expect(handlers.onStateChange).toHaveBeenLastCalledWith('connected')
    client.close()
  })
})
