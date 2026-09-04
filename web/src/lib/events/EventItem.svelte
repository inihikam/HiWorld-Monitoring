<script>
  /**
   * EventItem (WE2, WE-AC-002/008): baris timeline + accordion detail.
   * Icon severity berwarna + icon kind + ringkasan + detail JSON terformat.
   */
  import Badge from '../m3/Badge.svelte'
  import { t } from '../i18n.svelte.js'
  import { fmtTime, fmtBytes } from '../format.js'
  import { navigate } from '../../lib/router.svelte.js'

  let { event } = $props()
  let open = $state(false)

  const SEV_COLOR = { info: 'primary', warning: 'warning', critical: 'error' }
  const KIND_ICON = {
    spike_cpu: '▲',
    spike_mem: '▲',
    disk_almost_full: '▼',
    agent_down: '⏻',
    agent_up: '⏼',
  }
  const SEV_LABEL = { info: 'sev.info', warning: 'sev.warning', critical: 'sev.critical' }
  const KIND_LABEL = {
    spike_cpu: 'kind.spike_cpu',
    spike_mem: 'kind.spike_mem',
    disk_almost_full: 'kind.disk_almost_full',
    agent_down: 'kind.agent_down',
    agent_up: 'kind.agent_up',
  }

  /** Ringkasan nilai dari detail (guard field dinamis). */
  const summaryText = $derived.by(() => {
    const d = event.detail ?? {}
    const parts = []
    if (d.value != null) {
      parts.push(typeof d.value === 'number' && d.value > 1024 * 1024 ? fmtBytes(d.value) : `${d.value}`)
    }
    if (d.baseline != null) parts.push(`baseline ${d.baseline}`)
    if (d.mount) parts.push(d.mount)
    return parts.join(' · ')
  })

  function seeHost() {
    navigate(`/host/${event.host}?at=${event.ts}`)
  }
</script>

<div class="item sev-{event.severity}" data-testid="event-{event.ts}-{event.kind}">
  <button class="row" onclick={() => (open = !open)} data-testid="event-row-{event.ts}">
    <span class="sev-icon" aria-label={t(SEV_LABEL[event.severity]) ?? event.severity}>
      <Badge color={SEV_COLOR[event.severity] ?? 'neutral'} />
    </span>
    <span class="kind-icon">{KIND_ICON[event.kind] ?? '•'}</span>
    <span class="time">{fmtTime(event.ts)}</span>
    <span class="host">{event.host}</span>
    <span class="kind">{t(KIND_LABEL[event.kind]) ?? event.kind}</span>
    <span class="subject">{event.subject}</span>
    <span class="summary">{summaryText}</span>
    <span class="chev" class:open>{open ? '▾' : '▸'}</span>
  </button>

  {#if open}
    <div class="detail" data-testid="event-detail-{event.ts}">
      <dl>
        {#each Object.entries(event.detail ?? {}) as [k, v] (k)}
          <dt>{k}</dt>
          <dd>{typeof v === 'number' && v > 1024 * 1024 ? fmtBytes(v) : v}</dd>
        {/each}
      </dl>
      <button class="see-host" onclick={seeHost} data-testid="see-host-{event.ts}">
        {t('events.see_host')} →
      </button>
    </div>
  {/if}
</div>

<style>
  .item {
    border-bottom: 1px solid var(--md-surface-container-highest);
  }
  .row {
    display: grid;
    grid-template-columns: 20px 20px 70px 90px 130px 1fr 1fr 20px;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 8px;
    background: none;
    border: none;
    color: var(--md-on-surface);
    font: var(--md-type-body);
    text-align: left;
    cursor: pointer;
    border-radius: var(--md-radius-sm);
  }
  .row:hover {
    background: color-mix(in srgb, var(--md-primary) 6%, transparent);
  }
  .kind-icon {
    color: var(--md-on-surface-variant);
    font-size: 13px;
  }
  .sev-critical .kind-icon {
    color: var(--md-error);
  }
  .time,
  .host {
    font: var(--md-font-mono);
    font-size: 12px;
    color: var(--md-on-surface-variant);
  }
  .kind {
    font: var(--md-type-label);
  }
  .subject,
  .summary {
    color: var(--md-on-surface-variant);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chev {
    color: var(--md-on-surface-variant);
    text-align: right;
  }
  .detail {
    padding: 8px 16px 12px 68px;
    background: var(--md-surface-container-lowest);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    margin: 0;
  }
  dt {
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
  }
  dd {
    margin: 0;
    font: var(--md-font-mono);
    font-size: 12px;
    color: var(--md-on-surface);
  }
  .see-host {
    margin-top: 8px;
    background: none;
    border: none;
    color: var(--md-primary);
    font: var(--md-type-label);
    cursor: pointer;
    padding: 0;
  }
  @media (max-width: 899px) {
    .row {
      grid-template-columns: 16px 16px 60px 1fr 16px;
    }
    .kind,
    .subject,
    .host {
      display: none;
    }
  }
</style>
