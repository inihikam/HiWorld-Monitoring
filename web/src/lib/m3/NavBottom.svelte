<script>
  /** M3 Navigation Bar (mobile bottom). API sama dengan NavRail. */
  let { items = [], active = '', onnavigate = null } = $props()
</script>

<nav class="m3-bottom" aria-label="Primary mobile">
  {#each items as it (it.href)}
    <a
      href={it.href}
      class="item"
      class:active={active === it.href}
      onclick={(e) => {
        if (onnavigate) {
          e.preventDefault()
          onnavigate(it.href)
        }
      }}
      data-testid="mnav-{it.href}"
    >
      <span class="indicator">
        {#if it.icon}<span class="icon">{it.icon}</span>{/if}
        {#if it.badge}<span class="badge">{it.badge}</span>{/if}
      </span>
      <span class="label">{it.label}</span>
    </a>
  {/each}
</nav>

<style>
  .m3-bottom {
    display: flex;
    background: var(--md-surface-container);
    height: 72px;
    box-shadow: var(--md-elev-2);
    position: sticky;
    bottom: 0;
    z-index: 10;
  }
  .item {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    text-decoration: none;
    color: var(--md-on-surface-variant);
    position: relative;
  }
  .item.active {
    color: var(--md-on-surface);
  }
  .indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 30px;
    border-radius: var(--md-radius-full);
    position: relative;
    font-size: 20px;
    transition: background-color var(--md-dur-short) var(--md-ease);
  }
  .item.active .indicator {
    background: var(--md-secondary-container, var(--md-primary-container));
  }
  .label {
    font: var(--md-type-label);
    font-size: 11px;
  }
  .badge {
    position: absolute;
    top: -2px;
    right: 6px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: var(--md-radius-full);
    background: var(--md-error);
    color: var(--md-on-error);
    font-size: 10px;
    line-height: 16px;
    text-align: center;
  }
</style>
