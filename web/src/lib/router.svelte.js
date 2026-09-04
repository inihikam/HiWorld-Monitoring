/**
 * Hash router minimal (WD7) — ADR WD-2.
 * route: '#/' → '/', '#/host/abc' → '/host/abc'.
 */

export function currentRoute() {
  const h = window.location.hash
  if (!h || h === '#') return '/'
  return h.slice(1) // '#/events' → '/events'
}

export function navigate(path) {
  window.location.hash = path.startsWith('#') ? path : `#${path}`
}

/** Reactive route state — module-level listener (terpasang sekali). */
export const route = $state({ path: currentRoute() })

if (typeof window !== 'undefined') {
  window.addEventListener('hashchange', () => {
    route.path = currentRoute()
  })
}

export function initRouter(onChange) {
  const handler = () => onChange(currentRoute())
  window.addEventListener('hashchange', handler)
  handler() // initial
  return () => window.removeEventListener('hashchange', handler)
}

/** Pecah route jadi segmen: '/host/web-01' → ['host', 'web-01'] */
export function segments(route) {
  return route.split('/').filter(Boolean)
}
