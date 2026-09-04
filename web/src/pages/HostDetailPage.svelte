<script>
  /**
   * HostDetailPage (WH3+WH4, WH-AC-001..005, WH-AC-012/013).
   * Header host + range selector + 3 chart bertumpuk + net + as-of offline.
   */
  import TimeChart from '../lib/detail/TimeChart.svelte'
  import ProcessTable from '../lib/detail/ProcessTable.svelte'
  import Badge from '../lib/m3/Badge.svelte'
  import { t } from '../lib/i18n.svelte.js'
  import { historyStore, loadHistory, parseHistory, appendSnapshot } from '../lib/stores/history.svelte.js'
  import { hostsStore, isStale } from '../lib/stores/hosts.svelte.js'
  import { fmtBytes, fmtTime } from '../lib/format.js'
  import { navigate } from '../lib/router.svelte.js'
  import { api } from '../lib/api.js'

  let { hostId } = $props()

  const RANGES = [
    { ms: 900_000, key: 'host.range_15m' },
    { ms: 3_600_000, key: 'host.range_1h' },
    { ms: 21_600_000, key: 'host.range_6h' },
    { ms: 86_400_000, key: 'host.range_24h' },
  ]
  let rangeMs = $state(3_600_000)

  const hostEntry = $derived(hostsStore.byHost[hostId])
  const online = $derived(hostEntry?.online ?? false)
  const s = $derived(hostEntry?.latest?.system)
  const diskMounts = $derived(Object.keys(historyStore.series.disks))
  const diskSeries = $derived(diskMounts.map((m) => ({ mount: m, data: historyStore.series.disks[m] })))
  const processes = $derived(hostEntry?.latest?.processes ?? [])

  // WE5: deep link ?at=<ts> (dari event detail) → range ±30 menit sekitar ts
  $effect(() => {
    const at = new URLSearchParams(window.location.hash.split('?')[1] ?? '').get('at')
    if (at) {
      const center = Number(at)
      if (Number.isFinite(center)) {
        historyStore.host = hostId
        historyStore.rangeMs = 3_600_000
        historyStore.loading = true
        historyStore.error = null
        api(`/api/history?host=${encodeURIComponent(hostId)}&from=${center - 1_800_000}&to=${center + 1_800_000}`)
          .then(async (res) => {
            if (res.ok) historyStore.series = parseHistory(await res.json())
          })
        return
      }
    }
    loadHistory(api, hostId, rangeMs)
  })

  function setRange(ms) {
    rangeMs = ms
  }

  const diskColor = (i) =>
    ['#a8c7fa', '#7ddb8a', '#dfbbdd', '#ffc46b', '#f2b8b5'][i % 5]
</script>

<div class="detail" data-testid="detail-page">
  <div class="head">
    <button class="back" onclick={() => navigate('/')} aria-label={t('common.back')}>←</button>
    <h1>{hostId}</h1>
    <span class="status">
      <Badge
        color={!online ? 'error' : isStale(hostEntry ?? {}) ? 'warning' : 'success'}
        pulse={online}
        testid="detail-status"
      />
      {online ? t('common.online') : t('common.offline')}
    </span>
    {#if !online && hostEntry?.latest}
      <span class="as-of" data-testid="as-of">
        {t('host.as_of', { time: fmtTime(hostEntry.latest.timestamp_ms) })}
      </span>
    {/if}
  </div>

  <div class="ranges" role="tablist">
    {#each RANGES as r (r.ms)}
      <button
        class="range"
        class:active={rangeMs === r.ms}
        role="tab"
        aria-selected={rangeMs === r.ms}
        data-testid="range-{r.ms}"
        onclick={() => setRange(r.ms)}
      >
        {t(r.key)}
      </button>
    {/each}
  </div>

  <section>
    <h2>{t('host.cpu')}</h2>
    <TimeChart data={historyStore.series.cpu} label="CPU" unit="%" max={100} testid="chart-cpu" />
  </section>

  <section>
    <h2>{t('host.ram')}</h2>
    <TimeChart data={historyStore.series.mem} label="RAM" unit="%" max={100} testid="chart-ram" />
  </section>

  <section>
    <h2>{t('host.disk')}</h2>
    {#each diskSeries as ds, i (ds.mount)}
      <div class="disk-row">
        <span class="mount">{ds.mount}</span>
        <TimeChart data={ds.data} label={ds.mount} unit="%" max={100} color={diskColor(i)} testid="chart-disk-{i}" />
      </div>
    {:else}
      <p class="none">{t('events.empty')}</p>
    {/each}
    {#if s?.disks?.length}
      <div class="mount-list" data-testid="mount-list">
        {#each s.disks as d (d.mount)}
          <span>{d.mount}: {fmtBytes(d.used_bytes)} / {fmtBytes(d.total_bytes)}</span>
        {/each}
      </div>
    {/if}
  </section>

  <section>
    <h2>{t('host.processes')}</h2>
    <ProcessTable processes={processes} />
  </section>

  {#if s}
    <div class="net" data-testid="net-cards">
      <div class="net-card">
        <span class="k">{t('host.net_rx')}</span>
        <span class="v">{fmtBytes(s.net?.rx_bytes)}</span>
      </div>
      <div class="net-card">
        <span class="k">{t('host.net_tx')}</span>
        <span class="v">{fmtBytes(s.net?.tx_bytes)}</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  h1 {
    font: var(--md-type-headline);
    color: var(--md-on-surface);
    margin: 0;
    flex: 1;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
  }
  .as-of {
    font: var(--md-type-label);
    color: var(--md-error);
  }
  .back {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--md-outline);
    background: var(--md-surface-container-high);
    color: var(--md-on-surface);
    cursor: pointer;
  }
  .ranges {
    display: flex;
    gap: 8px;
  }
  .range {
    height: 32px;
    padding: 0 16px;
    border-radius: var(--md-radius-full);
    border: 1px solid var(--md-outline);
    background: transparent;
    color: var(--md-on-surface-variant);
    cursor: pointer;
    font: var(--md-type-label);
  }
  .range.active {
    background: var(--md-primary);
    color: var(--md-on-primary);
    border-color: var(--md-primary);
  }
  section h2 {
    font: var(--md-type-title);
    color: var(--md-on-surface);
    margin: 8px 0 4px;
  }
  .disk-row {
    margin-bottom: 8px;
  }
  .mount {
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
  }
  .mount-list {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
  }
  .none {
    color: var(--md-on-surface-variant);
    font: var(--md-type-body);
  }
  .net {
    display: flex;
    gap: 12px;
  }
  .net-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: var(--md-surface-container-low);
    border-radius: var(--md-radius-md);
    padding: 12px 20px;
  }
  .net-card .k {
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
  }
  .net-card .v {
    font: var(--md-type-title);
    color: var(--md-on-surface);
  }
</style>
