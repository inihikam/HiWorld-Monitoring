/**
 * WsClient (WD9, WD-AC-013/014/015).
 * Kontrak ws-broadcast:
 *   pesan pertama = hello {hosts:[{host_id, latest|null}]}
 *   berikutnya    = snapshot | event | host_status
 * Reconnect backoff 1s→2s→4s→cap 30s; status via callback.
 */

const BACKOFF = [1000, 2000, 4000, 8000, 16000, 30000] // cap 30s

export function createWsClient({ onHello, onSnapshot, onEvent, onHostStatus, onStateChange }) {
  let ws = null
  let closedByUser = false
  let attempt = 0
  let failStreak = 0
  let timer = null
  let pollFallback = null // dipasang penelepon bila WS gagal terus (WO5)

  function setStatus(s) {
    onStateChange?.(s)
  }

  function connect() {
    if (closedByUser) return
    setStatus(attempt === 0 ? 'connecting' : 'reconnecting')
    const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
    ws = new WebSocket(`${proto}://${window.location.host}/ws`)

    ws.onopen = () => {
      attempt = 0
      failStreak = 0
      setStatus('connected')
      stopPollFallback()
    }

    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data)
        switch (msg.type) {
          case 'hello':
            onHello?.(msg.data)
            break
          case 'snapshot':
            onSnapshot?.(msg.data)
            break
          case 'event':
            onEvent?.(msg.data)
            break
          case 'host_status':
            onHostStatus?.(msg.data)
            break
        }
      } catch {
        /* pesan korup — abaikan */
      }
    }

    ws.onclose = () => {
      if (closedByUser) return
      setStatus('reconnecting')
      scheduleReconnect()
    }

    ws.onerror = () => {
      ws?.close()
    }
  }

  function scheduleReconnect() {
    failStreak++
    const delay = BACKOFF[Math.min(attempt, BACKOFF.length - 1)]
    attempt++
    timer = setTimeout(() => {
      if (failStreak >= 3) startPollFallback()
      connect()
    }, delay)
  }

  /** Fallback polling (WO-AC-011, Should): 5s via REST /api/hosts. */
  function startPollFallback() {
    if (pollFallback) return
    pollFallback = setInterval(async () => {
      try {
        const res = await fetch('/api/hosts', { credentials: 'same-origin' })
        if (res.ok) {
          const data = await res.json()
          // bentuk host list → onHostStatus per host (fallback terbatas)
          for (const h of data.hosts ?? []) {
            onHostStatus?.({ host_id: h.host_id, online: !!h.online })
          }
        }
      } catch {
        /* tetap gagal — tunggu WS pulih */
      }
    }, 5000)
  }

  function stopPollFallback() {
    if (pollFallback) {
      clearInterval(pollFallback)
      pollFallback = null
    }
  }

  return {
    connect,
    close() {
      closedByUser = true
      clearTimeout(timer)
      stopPollFallback()
      ws?.close()
      setStatus('closed')
    },
    get status() {
      return closedByUser ? 'closed' : failStreak >= 3 ? 'fallback' : attempt > 0 ? 'reconnecting' : 'connecting'
    },
  }
}
