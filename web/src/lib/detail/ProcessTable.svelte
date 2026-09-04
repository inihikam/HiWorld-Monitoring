<script>
  /**
   * ProcessTable (WH5, WH-AC-008..011): top proses + sort + expand + bar mini.
   * props processes: [{pid, name, unit, cpu_percent, mem_percent, rss_bytes}]
   */
  import { t } from '../i18n.svelte.js'
  import { fmtBytes, fmtPercent } from '../format.js'
  import Button from '../m3/Button.svelte'

  let { processes = [] } = $props()

  let sortKey = $state('cpu_percent')
  let sortAsc = $state(false)
  let expanded = $state(false)

  const TOP_N = 15

  const sorted = $derived(
    [...processes].sort((a, b) => {
      const va = a[sortKey] ?? 0
      const vb = b[sortKey] ?? 0
      return sortAsc ? va - vb : vb - va
    })
  )
  const visible = $derived(expanded ? sorted : sorted.slice(0, TOP_N))

  function setSort(key) {
    if (sortKey === key) {
      sortAsc = !sortAsc
    } else {
      sortKey = key
      sortAsc = false // default desc utk angka
    }
  }

  const barColor = (v, high) => (high ? 'var(--md-error)' : 'var(--md-primary)')
</script>

<div class="ptable" data-testid="process-table">
  {#if !processes.length}
    <p class="none" data-testid="process-empty">{t('host.process_empty')}</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th><button class="th" onclick={() => setSort('pid')} data-testid="sort-pid">{t('col.pid')}</button></th>
          <th><button class="th" onclick={() => setSort('name')} data-testid="sort-name">{t('col.name')}</button></th>
          <th class="hide-mobile"><button class="th" onclick={() => setSort('unit')} data-testid="sort-unit">{t('col.unit')}</button></th>
          <th><button class="th active" onclick={() => setSort('cpu_percent')} data-testid="sort-cpu">{t('col.cpu')}</button></th>
          <th><button class="th" onclick={() => setSort('mem_percent')} data-testid="sort-ram">{t('col.ram')}</button></th>
          <th class="hide-mobile"><button class="th" onclick={() => setSort('rss_bytes')} data-testid="sort-rss">{t('col.rss')}</button></th>
        </tr>
      </thead>
      <tbody>
        {#each visible as p (p.pid)}
          <tr data-testid="proc-{p.pid}">
            <td class="mono">{p.pid}</td>
            <td>{p.name}</td>
            <td class="unit hide-mobile">{p.unit || '—'}</td>
            <td>
              <div class="cell" class:high={(p.cpu_percent ?? 0) >= 85}>
                <span class="mini" style:width="{Math.min(p.cpu_percent ?? 0, 100)}%"
                  style:background={barColor(p.cpu_percent, (p.cpu_percent ?? 0) >= 85)}></span>
                {fmtPercent(p.cpu_percent)}
              </div>
            </td>
            <td>
              <div class="cell" class:high={(p.mem_percent ?? 0) >= 85}>
                <span class="mini" style:width="{Math.min(p.mem_percent ?? 0, 100)}%"></span>
                {fmtPercent(p.mem_percent)}
              </div>
            </td>
            <td class="mono hide-mobile">{fmtBytes(p.rss_bytes)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if sorted.length > TOP_N}
      <div class="expand">
        <Button variant="text" onclick={() => (expanded = !expanded)} data-testid="expand-btn">
          {expanded ? t('host.processes_top15') : t('host.processes_all')} ({sorted.length})
        </Button>
      </div>
    {/if}
  {/if}
</div>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
    font: var(--md-type-body);
  }
  th {
    text-align: left;
    padding: 4px 8px;
    border-bottom: 1px solid var(--md-outline-variant);
  }
  .th {
    background: none;
    border: none;
    color: var(--md-on-surface-variant);
    font: var(--md-type-label);
    cursor: pointer;
    padding: 4px 0;
  }
  .th.active {
    color: var(--md-primary);
  }
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--md-surface-container-highest);
    color: var(--md-on-surface);
  }
  tr:hover td {
    background: color-mix(in srgb, var(--md-primary) 6%, transparent);
  }
  .mono {
    font: var(--md-font-mono);
    font-size: 12px;
  }
  .unit {
    color: var(--md-on-surface-variant);
    font-size: 12px;
  }
  .cell {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 72px;
    padding-left: 4px;
  }
  .mini {
    position: absolute;
    left: 0;
    top: 2px;
    bottom: 2px;
    border-radius: 2px;
    opacity: 0.25;
    background: var(--md-primary);
    transition: width var(--md-dur-med) var(--md-ease);
  }
  .cell.high {
    color: var(--md-error);
  }
  .none {
    color: var(--md-on-surface-variant);
  }
  .expand {
    display: flex;
    justify-content: center;
    margin-top: 8px;
  }
  @media (max-width: 899px) {
    .hide-mobile {
      display: none;
    }
  }
</style>
