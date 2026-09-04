<script>
  /**
   * M3 Navigation Rail (desktop sidebar).
   * items: [{ href, icon, label, badge? }]
   */
  let { items = [], active = '', onnavigate = null } = $props()
</script>

<nav class="m3-rail" aria-label="Primary">
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
      data-testid="nav-{it.href}"
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
  .m3-rail {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 88px;
    padding: 12px 8px;
    background: var(--md-surface-container-low);
    height: 100vh;
    box-sizing: border-box;
  }
  .item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    text-decoration: none;
    color: var(--md-on-surface-variant);
    border-radius: var(--md-radius-md);
    position: relative;
    transition: background-color var(--md-dur-short) var(--md-ease);
  }
  .item:hover {
    background: color-mix(in srgb, var(--md-primary) var(--md-state-hover), transparent);
  }
  .item.active {
    color: var(--md-on-surface);
  }
  .indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 32px;
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
    font-size: 12px;
  }
  .badge {
    position: absolute;
    top: 0;
    right: 8px;
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
