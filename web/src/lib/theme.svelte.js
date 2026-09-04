/**
 * Theme state (WD4, WD-AC-004/005/007).
 * Initial: localStorage → prefers-color-scheme → dark (default).
 * Persist: localStorage 'hiworld-theme'.
 */

const STORAGE_KEY = 'hiworld-theme'

function detectInitial() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved === 'dark' || saved === 'light') return saved
  } catch {
    /* localStorage unavailable — fall through */
  }
  if (typeof window !== 'undefined' && window.matchMedia) {
    if (window.matchMedia('(prefers-color-scheme: light)').matches) return 'light'
  }
  return 'dark' // ADR WD-4: default dark
}

export const theme = $state({ current: detectInitial() })

/** Terapkan tema ke <html data-theme> — dipanggil di App init. */
export function applyTheme() {
  if (typeof document !== 'undefined') {
    document.documentElement.dataset.theme = theme.current
  }
}

export function toggleTheme() {
  theme.current = theme.current === 'dark' ? 'light' : 'dark'
  try {
    localStorage.setItem(STORAGE_KEY, theme.current)
  } catch {
    /* non-fatal */
  }
  applyTheme()
  return theme.current
}
