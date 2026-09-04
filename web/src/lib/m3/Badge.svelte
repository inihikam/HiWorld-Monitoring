<script>
  /**
   * M3 Badge / status dot — color: success | warning | error | neutral | primary.
   * mode: dot (bulatan) | chip (teks kecil).
   */
  let { color = 'neutral', mode = 'dot', label = '', pulse = false, testid = null } = $props()

  const COLORS = {
    success: 'var(--md-success)',
    warning: 'var(--md-warning)',
    error: 'var(--md-error)',
    neutral: 'var(--md-outline)',
    primary: 'var(--md-primary)',
  }
</script>

{#if mode === 'dot'}
  <span
    class="dot"
    style:background={COLORS[color]}
    class:pulse
    data-testid={testid}
    title={label}
  ></span>
{:else}
  <span
    class="chip"
    style:background={COLORS[color]}
    data-testid={testid}
  >{label}</span>
{/if}

<style>
  .dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot.pulse {
    animation: pulse 1.6s infinite var(--md-ease);
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }
  .chip {
    display: inline-block;
    padding: 2px 10px;
    border-radius: var(--md-radius-full);
    font: var(--md-type-label);
    font-size: 11px;
    color: var(--md-on-primary);
    line-height: 1.5;
  }
</style>
