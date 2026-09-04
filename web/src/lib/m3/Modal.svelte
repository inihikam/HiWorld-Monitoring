<script>
  /** M3 Modal sederhana (dipakai empty state overview & lainnya). */
  import Button from './Button.svelte'
  import { t } from '../i18n.svelte.js'
  let { title = '', children, onclose = null, testid = null } = $props()
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose?.()} />

<div class="backdrop" data-testid={testid} onclick={(e) => e.target === e.currentTarget && onclose?.()}>
  <div class="dialog" role="dialog" aria-label={title}>
    <h2>{title}</h2>
    <div class="body">{@render children?.()}</div>
    <div class="actions">
      <Button variant="text" onclick={onclose}>{t('common.close')}</Button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: grid;
    place-items: center;
    z-index: 100;
  }
  .dialog {
    background: var(--md-surface-container-high);
    border-radius: var(--md-radius-xl);
    padding: 24px;
    width: min(420px, 90vw);
    box-shadow: var(--md-elev-3);
  }
  h2 {
    font: var(--md-type-headline);
    color: var(--md-on-surface);
    margin: 0 0 12px;
  }
  .body {
    font: var(--md-type-body);
    color: var(--md-on-surface-variant);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 16px;
  }
</style>
