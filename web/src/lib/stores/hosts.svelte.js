/**
 * hostsStore (WD9 — fondasi WO1): state host realtime dari WS.
 * byHost: { [host_id]: { latest: Snapshot|null, online: bool } }
 */

export const hostsStore = $state({ byHost: {} })

/** hello bootstrap: { hosts: [{host_id, latest|null}] } */
export function applyHello(data) {
  const next = {}
  for (const h of data.hosts ?? []) {
    next[h.host_id] = { latest: h.latest ?? null, online: h.latest != null }
  }
  hostsStore.byHost = next
}

/** WS snapshot → update satu host. */
export function applySnapshot(snap) {
  const cur = hostsStore.byHost[snap.host_id] ?? { latest: null, online: false }
  hostsStore.byHost[snap.host_id] = { ...cur, latest: snap, online: true }
}

/** WS host_status {host_id, online} → update badge (data dibekukan). */
export function applyStatus(st) {
  const cur = hostsStore.byHost[st.host_id]
  if (cur) {
    hostsStore.byHost[st.host_id] = { ...cur, online: !!st.online }
  }
}
