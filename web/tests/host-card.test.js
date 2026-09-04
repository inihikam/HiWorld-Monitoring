import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/svelte'
import HostCard from '../src/lib/overview/HostCard.svelte'

function host(over = {}) {
  return {
    host_id: 'web-01',
    latest: {
      host_id: 'web-01',
      timestamp_ms: 1_000,
      system: {
        cpu_percent: 42,
        mem_percent: 50,
        mem_used_bytes: 1024 ** 3 * 2,
        mem_total_bytes: 1024 ** 3 * 8,
        load_avg: [0.5, 0.4, 0.3],
        disks: [
          { mount: '/', used_percent: 30 },
          { mount: '/data', used_percent: 91 },
        ],
      },
    },
    online: true,
    lastSeenMs: Date.now(),
    ...over,
  }
}

describe('HostCard (WO2)', () => {
  it('render nama, nilai CPU/RAM, disk tertinggi + mount (WO-AC-001)', () => {
    render(HostCard, { props: { host: host() } })
    expect(screen.getByText('web-01')).toBeInTheDocument()
    expect(screen.getByTestId('cpu-web-01')).toHaveTextContent('42.0%')
    expect(screen.getByTestId('ram-web-01')).toHaveTextContent('50.0%')
    // disk tertinggi = /data 91%, bukan / 30%
    expect(screen.getByTestId('disk-web-01')).toHaveTextContent('/data')
    expect(screen.getByTestId('disk-web-01')).toHaveTextContent('91.0%')
  })

  it('disk ≥90 → class high; CPU/RAM <85 → tidak (WO-AC-008)', () => {
    const { container } = render(HostCard, { props: { host: host() } })
    const metrics = container.querySelectorAll('.metric')
    expect(metrics[0].className).not.toContain('high') // cpu 42
    expect(metrics[1].className).not.toContain('high') // ram 50
    expect(metrics[2].className).toContain('high') // disk 91
  })

  it('CPU ≥85 → high (WO-AC-008)', () => {
    const { container } = render(HostCard, {
      props: { host: host({ latest: { ...host().latest, system: { ...host().latest.system, cpu_percent: 88 } } }) },
    })
    expect(container.querySelectorAll('.metric')[0].className).toContain('high')
  })

  it('klik kartu → navigate #/host/:id (WO-AC-003)', async () => {
    render(HostCard, { props: { host: host() } })
    await fireEvent.click(screen.getByTestId('host-card-web-01'))
    expect(window.location.hash).toBe('#/host/web-01')
  })

  it('offline → badge error + tetap tampil data terakhir', () => {
    render(HostCard, {
      props: { host: host({ online: false }) },
    })
    expect(screen.getByTestId('status-web-01')).toBeInTheDocument()
    expect(screen.getByTestId('cpu-web-01')).toHaveTextContent('42.0%') // beku
  })

  it('latest null → waiting first sample (WO-AC-006)', () => {
    render(HostCard, {
      props: { host: host({ latest: null, online: false, lastSeenMs: null }) },
    })
    expect(screen.getByTestId('waiting-web-01')).toBeInTheDocument()
  })
})
