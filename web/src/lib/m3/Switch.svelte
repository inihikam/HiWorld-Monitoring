<script>
  /** M3 Switch. */
  let { checked = $bindable(false), label = '', disabled = false, testid = null } = $props()
</script>

<label class="m3-switch" class:disabled>
  <input
    type="checkbox"
    bind:checked
    {disabled}
    data-testid={testid}
  />
  <span class="track"><span class="thumb"></span></span>
  {#if label}<span class="label">{label}</span>{/if}
</label>

<style>
  .m3-switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }
  .m3-switch.disabled {
    opacity: 0.38;
    cursor: not-allowed;
  }
  input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }
  .track {
    width: 52px;
    height: 32px;
    border-radius: var(--md-radius-full);
    background: var(--md-surface-container-highest);
    border: 2px solid var(--md-outline);
    display: inline-flex;
    align-items: center;
    padding: 2px;
    transition:
      background-color var(--md-dur-short) var(--md-ease),
      border-color var(--md-dur-short) var(--md-ease);
    box-sizing: border-box;
  }
  .thumb {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--md-outline);
    transition: all var(--md-dur-short) var(--md-ease);
  }
  input:checked + .track {
    background: var(--md-primary);
    border-color: var(--md-primary);
  }
  input:checked + .track .thumb {
    width: 24px;
    background: var(--md-on-primary);
  }
  input:focus-visible + .track {
    outline: 2px solid var(--md-primary);
    outline-offset: 2px;
  }
  .label {
    font: var(--md-type-body);
    color: var(--md-on-surface);
  }
</style>
