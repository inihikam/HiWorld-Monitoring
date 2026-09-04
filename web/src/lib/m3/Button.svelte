<script>
  /**
   * M3 Button (filled) — variants: filled (default), tonal, text, outlined.
   * WD-AC-016/017: state layer hover/press.
   */
  let { variant = 'filled', disabled = false, type = 'button', children, onclick, ...rest } = $props()
</script>

<button
  {type}
  class="m3-btn {variant}"
  {disabled}
  {onclick}
  {...rest}
>
  {@render children?.()}
</button>

<style>
  .m3-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 40px;
    padding: 0 24px;
    border: none;
    border-radius: var(--md-radius-full);
    font: var(--md-type-label);
    font-size: 14px;
    cursor: pointer;
    position: relative;
    overflow: hidden;
    transition:
      box-shadow var(--md-dur-short) var(--md-ease),
      background-color var(--md-dur-short) var(--md-ease);
  }
  .m3-btn::after {
    /* state layer */
    content: '';
    position: absolute;
    inset: 0;
    background: currentColor;
    opacity: 0;
    transition: opacity var(--md-dur-short) var(--md-ease);
    pointer-events: none;
  }
  .m3-btn:hover::after {
    opacity: var(--md-state-hover);
  }
  .m3-btn:active::after {
    opacity: var(--md-state-press);
  }
  .m3-btn:disabled {
    opacity: 0.38;
    cursor: not-allowed;
  }

  .filled {
    background: var(--md-primary);
    color: var(--md-on-primary);
  }
  .tonal {
    background: var(--md-secondary);
    color: var(--md-on-secondary);
  }
  .text {
    background: transparent;
    color: var(--md-primary);
    padding: 0 12px;
  }
  .outlined {
    background: transparent;
    color: var(--md-primary);
    border: 1px solid var(--md-outline);
  }
</style>
