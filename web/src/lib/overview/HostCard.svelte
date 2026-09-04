<script>
  /**
   * HostCard (WO2, WO-AC-001/003/008): kartu host realtime.
   * prop host: { host_id, latest, online, lastSeenMs }
   * Klik → navigate #/host/:id. Threshold warna: CPU/RAM ≥85, disk ≥90.
   */
  import Badge from '../m3/Badge.svelte'
  import { t } from '../i18n.svelte.js'
  import { fmtBytes, fmtPercent, fmtLoad } from '../format.js'
  import { isStale } from '../stores/hosts.svelte.js'
  import { navigate } from '../../lib/router.svelte.js'

  let { host } = $props()

  const s = $derived(host.latest?.system)
  const status = $derived(
    !host.online ? 'offline' : isStale(host) ? 'stale' : host.latest ? 'online' : 'waiting'
  )
  const statusColor = $derived(
    status === 'offline' ? 'error' : status === 'stale' ? 'warning' : 'success'
  )
  const statusLabel = $derived(
    status === 'offline'
      ? t('common.offline')
      : status === 'stale'
        ? t('common.stale')
        : status === 'waiting'
          ? t('common.waiting_first_sample')
          : t('common.online')
  )

  function topDisk(system) {
    if (!system?.disks?.length) return null
    return system.disks.reduce((a, b) => (b.used_percent > a.used_percent ? b : a))
  }
  const disk = $derived(topDisk(s))

  const cpuHigh = $derived((s?.cpu_percent ?? 0) >= 85)
  const ramHigh = $derived((s?.mem_percent ?? 0) >= 85)
  const diskHigh = $derived((disk?.used_percent ?? 0) >= 90)
</script>

<div
  class="card"
  role="button"
  tabindex="0"
  data-testid="host-card-{host.host_id}"
  onclick={() => navigate(`/host/${host.host_id}`)}
  onkeydown={(e) => e.key === 'Enter' && navigate(`/host/${host.host_id}`)}
>
  <div class="head">
    <span class="name">{host.host_id}</span>
    <span class="status">
      <Badge color={statusColor} pulse={status === 'online'} testid="status-{host.host_id}" />
      {statusLabel}
    </span>
  </div>

  {#if host.latest}
    <div class="rows">
      <div class="metric" class:high={cpuHigh}>
        <span class="label">CPU</span>
        <div class="bar"><div class="fill" style:width="{Math.min(s.cpu_percent ?? 0, 100)}%"></div></div>
        <span class="value" data-testid="cpu-{host.host_id}">{fmtPercent(s?.cpu_percent)}</span>
      </div>
      <div class="metric" class:high={ramHigh}>
        <span class="label">RAM</span>
        <div class="bar"><div class="fill" style:width="{Math.min(s?.mem_percent ?? 0, 100)}%"></div></div>
        <span class="value" data-testid="ram-{host.host_id}">{fmtPercent(s?.mem_percent)}</span>
      </div>
      <div class="metric" class:high={diskHigh}>
        <span class="label">{t('host.disk')}</span>
        <div class="bar"><div class="fill" style:width="{Math.min(disk?.used_percent ?? 0, 100)}%"></div></div>
        <span class="value" data-testid="disk-{host.host_id}">
          {disk ? `${fmtPercent(disk.used_percent)} · ${disk.mount}` : '—'}
        </span>
      </div>
      <div class="sub">
        <span>
          {t('host.ram')}:
          {s ? `${fmtBytes(s.mem_used_bytes)} / ${fmtBytes(s.mem_total_bytes)}` : '—'}
        </span>
        <span>{t('host.load_1m')}: {fmtLoad(s?.load_avg?.[0])}</span>
      </div>
    </div>
  {:else}
    <p class="waiting" data-testid="waiting-{host.host_id}">{t('common.waiting_first_sample')}</p>
  {/if}
</div>

<style>
  .card {
    background: var(--md-surface-container-low);
    border-radius: var(--md-radius-md);
    padding: 16px;
    box-shadow: var(--md-elev-1);
    cursor: pointer;
    transition:
      box-shadow var(--md-dur-short) var(--md-ease),
      background-color var(--md-dur-short) var(--md-ease);
  }
  .card:hover {
    background: var(--md-surface-container);
    box-shadow: var(--md-elev-2);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  .name {
    font: var(--md-type-title);
    color: var(--md-on-surface);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: var(--md-type-label);
    font-size: 12px;
    color: var(--md-on-surface-variant);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .metric {
    display: grid;
    grid-template-columns: 42px 1fr 74px;
    align-items: center;
    gap: 8px;
  }
  .label {
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
  }
  .bar {
    height: 6px;
    border-radius: var(--md-radius-full);
    background: var(--md-surface-container-highest);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--md-primary);
    transition: width var(--md-dur-med) var(--md-ease);
  }
  .metric.high .fill {
    background: var(--md-error);
  }
  .metric.high .value {
    color: var(--md-error);
  }
  .value {
    font: var(--md-type-label);
    font-size: 12px;
    color: var(--md-on-surface);
    text-align: right;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    display: flex;
    justify-content: space-between;
    font: var(--md-type-label);
    font-size: 11px;
    color: var(--md-on-surface-variant);
    margin-top: 4px;
  }
  .waiting {
    font: var(--md-type-body);
    color: var(--md-on-surface-variant);
    margin: 8px 0 0;
  }
</style>
