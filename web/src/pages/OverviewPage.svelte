<script>
  /**
   * OverviewPage (WO3+WO4, WO-AC-002..007, WO-AC-009/010).
   * Grid kartu host realtime + ring summary + empty state + sorting Q-WO1.
   */
  import HostCard from '../lib/overview/HostCard.svelte'
  import Button from '../lib/m3/Button.svelte'
  import Modal from '../lib/m3/Modal.svelte'
  import { t } from '../lib/i18n.svelte.js'
  import { hostsStore, sortedHosts, summary } from '../lib/stores/hosts.svelte.js'
  import { fmtPercent } from '../lib/format.js'

  let showSetup = $state(false)

  const ordered = $derived(sortedHosts(hostsStore.byHost))
  const agg = $derived(summary(hostsStore.byHost))
  const hasAny = $derived(Object.keys(hostsStore.byHost).length > 0)
</script>

<div class="overview" data-testid="overview-page">
  <div class="summary" data-testid="ring-summary">
    <span class="agg">
      {agg.online}/{agg.total} {t('overview.hosts_online', { online: '', total: '' }).trim().replace(/of|dari/, '') || ''}
    </span>
    <span class="agg">
      {t('overview.avg_cpu')}: <strong data-testid="avg-cpu">{fmtPercent(agg.avgCpu)}</strong>
    </span>
    <span class="agg">
      {t('overview.avg_ram')}: <strong data-testid="avg-ram">{fmtPercent(agg.avgRam)}</strong>
    </span>
  </div>

  {#if !hasAny}
    <div class="empty" data-testid="empty-state">
      <p>{t('events.empty') !== 'Belum ada event' ? '' : ''}</p>
      <p class="empty-title">No hosts yet</p>
      <p class="empty-body">Run hiworld-agent on your servers; they will register here automatically.</p>
      <Button onclick={() => (showSetup = true)}>{t('overview.setup_agent')}</Button>
    </div>
  {:else}
    <div class="grid">
      {#each ordered as host (host.host_id)}
        <HostCard {host} />
      {/each}
    </div>
  {/if}
</div>

{#if showSetup}
  <Modal title={t('overview.setup_title')} onclose={() => (showSetup = false)} testid="setup-modal">
    <p>{t('overview.setup_body')}</p>
  </Modal>
{/if}

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .summary {
    display: flex;
    gap: 24px;
    padding: 12px 16px;
    background: var(--md-surface-container-low);
    border-radius: var(--md-radius-md);
    font: var(--md-type-body);
    color: var(--md-on-surface-variant);
  }
  .agg strong {
    color: var(--md-on-surface);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 48px 16px;
    background: var(--md-surface-container-lowest);
    border-radius: var(--md-radius-lg);
  }
  .empty-title {
    font: var(--md-type-headline);
    color: var(--md-on-surface);
    margin: 0;
  }
  .empty-body {
    font: var(--md-type-body);
    color: var(--md-on-surface-variant);
    margin: 0;
    max-width: 420px;
    text-align: center;
  }
</style>
