import '@testing-library/jest-dom/vitest'

// jsdom tidak punya matchMedia — stub global (App pakai utk breakpoint)
window.matchMedia = window.matchMedia || ((q) => ({
  matches: false,
  media: q,
  addEventListener: () => {},
  removeEventListener: () => {},
}))
