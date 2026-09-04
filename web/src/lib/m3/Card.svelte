<script>
  /** M3 Card — elevated container. */
  let { children, onclick = null, testid = null } = $props()
</script>

<div
  class="m3-card"
  role={onclick ? 'button' : undefined}
  tabindex={onclick ? 0 : undefined}
  {onclick}
  data-testid={testid}
>
  {@render children?.()}
</div>

<style>
  .m3-card {
    background: var(--md-surface-container-low);
    border-radius: var(--md-radius-md);
    padding: 16px;
    box-shadow: var(--md-elev-1);
    transition:
      box-shadow var(--md-dur-short) var(--md-ease),
      background-color var(--md-dur-short) var(--md-ease);
  }
  .m3-card[role='button'] {
    cursor: pointer;
  }
  .m3-card[role='button']:hover {
    background: var(--md-surface-container);
    box-shadow: var(--md-elev-2);
  }
</style>
