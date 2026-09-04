import { vi } from 'vitest'
import '@testing-library/jest-dom/vitest'

// jsdom tidak punya ResizeObserver (TimeChart pakai)
class MockResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}
window.ResizeObserver = window.ResizeObserver || MockResizeObserver

// jsdom tidak punya matchMedia — stub global (App pakai utk breakpoint)
window.matchMedia = window.matchMedia || ((q) => ({
  matches: false,
  media: q,
  addEventListener: () => {},
  removeEventListener: () => {},
}))

// Default fetch stub untuk test shell (App probe auth saat init).
// Test auth spesifik override dengan vi.stubGlobal sendiri.
if (!window.fetch.__isTestStub) {
  window.fetch = Object.assign(vi.fn().mockResolvedValue({ status: 200, ok: true }), {
    __isTestStub: true,
  })
}
