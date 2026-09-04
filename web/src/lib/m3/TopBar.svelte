<script>
  /** Top-bar aplikasi: judul kiri, slot actions kanan. */
  let { title = '', children } = $props()
</script>

<header class="m3-topbar">
  <span class="title">{title}</span>
  <div class="actions">
    {@render children?.()}
  </div>
</header>

<style>
  .m3-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 64px;
    padding: 0 16px;
    background: var(--md-surface-container);
    color: var(--md-on-surface);
    box-shadow: var(--md-elev-1);
    position: sticky;
    top: 0;
    z-index: 10;
  }
  .title {
    font: var(--md-type-title);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
