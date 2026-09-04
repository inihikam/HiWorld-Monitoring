import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'
import ProcessTable from '../src/lib/detail/ProcessTable.svelte'

function procs(n) {
  return Array.from({ length: n }, (_, i) => ({
    pid: i + 1,
    name: `proc-${i + 1}`,
    unit: i % 2 ? 'nginx.service' : null,
    cpu_percent: 90 - i * 2,
    mem_percent: 5 + i,
    rss_bytes: 1024 ** 2 * (100 - i),
  }))
}

describe('ProcessTable (WH5)', () => {
  it('render baris + kolom (WH-AC-008)', () => {
    render(ProcessTable, { props: { processes: procs(3) } })
    expect(screen.getByTestId('proc-1')).toBeInTheDocument()
    expect(screen.getByTestId('proc-1')).toHaveTextContent('proc-1')
    expect(screen.getByTestId('proc-1')).toHaveTextContent('90.0%')
  })

  it('default sort CPU desc, top 15 (WH-AC-008)', () => {
    render(ProcessTable, { props: { processes: procs(20) } })
    const rows = screen.getAllByTestId(/^proc-/)
    expect(rows.length).toBe(15)
    expect(rows[0]).toHaveTextContent('proc-1') // cpu 90 tertinggi
  })

  it('expand show all (WH-AC-008)', async () => {
    render(ProcessTable, { props: { processes: procs(20) } })
    await fireEvent.click(screen.getByTestId('expand-btn'))
    await waitFor(() => {
      expect(screen.getAllByTestId(/^proc-/).length).toBe(20)
    })
  })

  it('klik header sort → urut kolom itu (WH-AC-010)', async () => {
    render(ProcessTable, { props: { processes: procs(5) } })
    await fireEvent.click(screen.getByTestId('sort-ram')) // mem asc? default desc
    await waitFor(() => {
      const rows = screen.getAllByTestId(/^proc-/)
      expect(rows[0]).toHaveTextContent('proc-5') // mem 5+4=9 tertinggi
    })
  })

  it('CPU ≥85 → class high (WH-AC-011 bar mini)', () => {
    const { container } = render(ProcessTable, { props: { processes: procs(5) } })
    expect(container.querySelector('[data-testid="proc-1"] .cell.high')).toBeTruthy()
  })

  it('kosong → empty state (WH-AC-008)', () => {
    render(ProcessTable, { props: { processes: [] } })
    expect(screen.getByTestId('process-empty')).toBeInTheDocument()
  })
})
