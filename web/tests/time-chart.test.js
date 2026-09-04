import { describe, it, expect, vi } from 'vitest'

// Mock uPlot — jsdom tidak bisa canvas; verifikasi kontrak setData saja.
vi.mock('uplot', () => {
  globalThis.__uplotInstances = []
  const instances = globalThis.__uplotInstances
  class MockUPlot {
    constructor(opts, data, target) {
      this.opts = opts
      this.data = data
      this.target = target
      this.setData = vi.fn()
      this.setSize = vi.fn()
      this.destroy = vi.fn()
      instances.push(this)
    }
  }
  return { default: MockUPlot }
})

import uPlot from 'uplot'
import { render, waitFor } from '@testing-library/svelte'
import TimeChart from '../src/lib/detail/TimeChart.svelte'

describe('TimeChart (WH2)', () => {
  it('mount → uPlot dibuat dengan data ter-transform', async () => {
    const data = [[1000, 10], [2000, 20]]
    const { unmount } = render(TimeChart, { props: { data, label: 'CPU', testid: 'chart' } })
    await waitFor(() => {
      expect(globalThis.__uplotInstances.length).toBeGreaterThan(0)
    })
    const inst = globalThis.__uplotInstances.at(-1)
    // transform: [[ts...],[v...]]
    expect(inst.data).toEqual([[1000, 2000], [10, 20]])
    expect(inst.opts.series[1].spanGaps).toBe(false) // WH-AC-005
    expect(inst.opts.cursor.sync.key).toBe('hiworld-host') // WH-AC-002 share-x
    unmount()
  })

  it('data berubah → setData dipanggil tanpa re-instantiate (WH-AC-004)', async () => {
    const { rerender } = render(TimeChart, {
      props: { data: [[1000, 5]], label: 'CPU', testid: 'c2' },
    })
    await waitFor(() => {
      expect(globalThis.__uplotInstances.length).toBeGreaterThan(0)
    })
    const inst = globalThis.__uplotInstances.at(-1)
    const count = globalThis.__uplotInstances.length
    await rerender({ data: [[1000, 5], [2000, 9]], label: 'CPU', testid: 'c2' })
    expect(inst.setData).toHaveBeenCalled()
    expect(globalThis.__uplotInstances.length).toBe(count) // tidak re-create
  })

  it('scales y range [0,max] (persen)', async () => {
    render(TimeChart, { props: { data: [[1, 5]], max: 100, testid: 'c3' } })
    await waitFor(() => {
      expect(globalThis.__uplotInstances.at(-1).opts.scales.y.range).toEqual([0, 100])
    })
  })
})
