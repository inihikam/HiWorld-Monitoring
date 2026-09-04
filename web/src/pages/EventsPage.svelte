<script>
  /**
   * EventsPage (WE2-WE4, WE-AC-005..010): timeline + filter + load more +
   * live badge. Deep link ?at= di sisi host detail (WE5).
   */
  import EventItem from '../lib/events/EventItem.svelte'
  import Button from '../lib/m3/Button.svelte'
  import { t } from '../lib/i18n.svelte.js'
  import {
    eventsStore,
    loadEvents,
    loadMore,
    filterEvents,
    groupByDay,
  } from '../lib/stores/events.svelte.js'
  import { api } from '../lib/api.js'

  const SEVS = ['info', 'warning', 'critical']
  const KINDS = ['spike_cpu', 'spike_mem', 'disk_almost_full', 'agent_down', 'agent_up']

  let sevSel = $state([])
  let kindSel = $state([])
  let search = $state('')
  let hostFilter = $state('')

  // init + WS handler pasang di App (eventsBadge) — di sini initial load
  $effect(() => {
    loadEvents(api)
  })

  const hosts = $derived([...new Set(eventsStore.list.map((e) => e.host))])
  const filtered = $derived(
    filterEvents(eventsStore.list, {
      severities: sevSel,
      kinds: kindSel,
      search: hostFilter ? `${hostFilter} ${search}` : search,
    }).filter((e) => !hostFilter || e.host === hostFilter)
  )
  const groups = $derived(groupByDay(filtered))

  function toggle(arr, v) {
    return arr.includes(v) ? arr.filter((x) => x !== v) : [...arr, v]
  }

  const DAY_LABEL = (label) =>
    label === '__today__' ? t('events.today') : label === '__yesterday__' ? t('events.yesterday') : label
</script>

<div class="events" data-testid="events-page">
  <div class="filters" data-testid="event-filters">
    <select bind:value={hostFilter} data-testid="filter-host" aria-label="Filter host">
      <option value="">{t('events.filter_host')}</option>
      {#each hosts as h (h)}
        <option value={h}>{h}</option>
      {/each}
    </select>

    <div class="chips">
      {#each SEVS as s (s)}
        <button
          class="chip sev-{s}"
          class:active={sevSel.includes(s)}
          data-testid="filter-sev-{s}"
          onclick={() => (sevSel = toggle(sevSel, s))}
        >{t(`sev.${s}`)}</button>
      {/each}
      {#each KINDS as k (k)}
        <button
          class="chip"
          class:active={kindSel.includes(k)}
          data-testid="filter-kind-{k}"
          onclick={() => (kindSel = toggle(kindSel, k))}
        >{t(`kind.${k}`)}</button>
      {/each}
    </div>

    <input
      class="search"
      type="search"
      placeholder="search…"
      bind:value={search}
      data-testid="filter-search"
    />
  </div>

  {#if !filtered.length}
    <p class="empty" data-testid="events-empty">{t('events.empty')}</p>
  {:else}
    {#each groups as g (g.label)}
      <h3 class="day">{DAY_LABEL(g.label)}</h3>
      {#each g.items as e (e.ts + e.kind + e.host)}
        <EventItem event={e} />
      {/each}
    {/each}

    <div class="more">
      <Button variant="text" onclick={() => { loadMore(); loadEvents(api) }} data-testid="load-more">
        {t('events.load_more')}
      </Button>
    </div>
  {/if}
</div>

<style>
  .events {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    margin-bottom: 8px;
  }
  select,
  .search {
    height: 32px;
    padding: 0 12px;
    border: 1px solid var(--md-outline);
    border-radius: var(--md-radius-full);
    background: var(--md-surface-container-lowest);
    color: var(--md-on-surface);
    font: var(--md-type-label);
  }
  .search {
    min-width: 180px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--md-radius-full);
    border: 1px solid var(--md-outline);
    background: transparent;
    color: var(--md-on-surface-variant);
    font: var(--md-type-label);
    font-size: 11px;
    cursor: pointer;
  }
  .chip.active.sev-info {
    background: var(--md-primary);
    color: var(--md-on-primary);
    border-color: var(--md-primary);
  }
  .chip.active.sev-warning {
    background: var(--md-warning);
    color: var(--md-on-warning);
    border-color: var(--md-warning);
  }
  .chip.active.sev-critical {
    background: var(--md-error);
    color: var(--md-on-error);
    border-color: var(--md-error);
  }
  .chip.active:not(.sev-info):not(.sev-warning):not(.sev-critical) {
    background: var(--md-secondary);
    color: var(--md-on-secondary);
  }
  .day {
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
    margin: 12px 0 4px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .empty {
    color: var(--md-on-surface-variant);
    text-align: center;
    padding: 32px;
  }
  .more {
    display: flex;
    justify-content: center;
    margin-top: 8px;
  }
</style>
