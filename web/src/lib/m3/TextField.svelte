<script>
  /** M3 Outlined TextField dengan label float. */
  let {
    label = '',
    value = $bindable(''),
    type = 'text',
    error = null,
    testid = null,
    onkeydown = null,
  } = $props()

  const id = `tf-${Math.random().toString(36).slice(2, 8)}`
</script>

<div class="m3-tf" class:has-error={!!error}>
  <input {id} {type} bind:value {onkeydown} placeholder=" " data-testid={testid} />
  <label for={id}>{label}</label>
  {#if error}
    <span class="err" data-testid="{testid}-error">{error}</span>
  {/if}
</div>

<style>
  .m3-tf {
    position: relative;
    display: inline-flex;
    flex-direction: column;
    gap: 4px;
    min-width: 240px;
  }
  input {
    height: 52px;
    padding: 0 16px;
    font: var(--md-type-body);
    color: var(--md-on-surface);
    background: var(--md-surface-container-lowest);
    border: 1px solid var(--md-outline);
    border-radius: var(--md-radius-xs);
    outline: none;
    transition: border-color var(--md-dur-short) var(--md-ease);
  }
  input:focus {
    border: 2px solid var(--md-primary);
    padding: 0 15px; /* kompensasi border 2px */
  }
  label {
    position: absolute;
    left: 12px;
    top: 16px;
    padding: 0 4px;
    font: var(--md-type-body);
    color: var(--md-on-surface-variant);
    background: var(--md-surface-container-lowest);
    pointer-events: none;
    transition: all var(--md-dur-short) var(--md-ease);
  }
  input:focus + label,
  input:not(:placeholder-shown) + label {
    top: -8px;
    font-size: 12px;
    color: var(--md-primary);
  }
  .has-error input {
    border-color: var(--md-error);
  }
  .has-error label {
    color: var(--md-error);
  }
  .err {
    font: var(--md-type-label);
    color: var(--md-error);
  }
</style>
