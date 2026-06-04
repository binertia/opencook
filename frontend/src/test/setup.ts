import '@testing-library/jest-dom'
import { vi } from 'vitest'

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
})

// Polyfills for Radix UI Select in jsdom
Element.prototype.setPointerCapture = Element.prototype.setPointerCapture || vi.fn()
Element.prototype.releasePointerCapture = Element.prototype.releasePointerCapture || vi.fn()
Element.prototype.hasPointerCapture = Element.prototype.hasPointerCapture || vi.fn(() => false)

// Polyfill for ResizeObserver (required by recharts)
class ResizeObserverMock {
  observe = vi.fn()
  unobserve = vi.fn()
  disconnect = vi.fn()
}
window.ResizeObserver = ResizeObserverMock
