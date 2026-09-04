<script>
  /**
   * TimeChart (WH2, WH-AC-002/005/006): wrapper uPlot.
   * - data: [[ts...], [v...]] format uPlot
   * - null → gap (spanGaps:false)
   * - crosshair + tooltip bawaan uPlot (legend off, cursor sync via group)
   * - resize observer utk width responsif
   */
  import { onMount, onDestroy } from 'svelte'
  import uPlot from 'uplot'
  import 'uplot/dist/uPlot.min.css'

  let {
    data = [[], []],
    label = '',
    unit = '%',
    max = 100,
    color = null,
    height = 180,
    testid = null,
  } = $props()

  let el
  let plot = null

  const opts = () => ({
    width: el?.clientWidth || 600,
    height,
    title: undefined,
    legend: { show: false },
    cursor: { sync: { key: 'hiworld-host' } }, // share-X sinkron (WH-AC-002)
    scales: { y: { range: [0, max] } },
    series: [
      {},
      {
        label: label || unit,
        stroke: color || getComputedStyle(document.documentElement).getPropertyValue('--md-primary') || '#a8c7fa',
        spanGaps: false, // null = gap (WH-AC-005)
        width: 1.5,
        fill: `${color || 'rgba(168,199,250,0.15)'}`,
        points: { show: false },
        value: (_u, v) => (v == null ? '—' : `${v.toFixed(1)}${unit}`), // WH-AC-006
      },
    ],
    axes: [
      { stroke: '#888', grid: { stroke: 'rgba(128,128,128,0.15)' } },
      {
        stroke: '#888',
        grid: { stroke: 'rgba(128,128,128,0.15)' },
        values: (_u, ticks) => ticks.map((v) => `${v}${unit}`),
      },
    ],
  })

  function toUplotData(d) {
    // d: [[ts,v],...] → uPlot [[ts...],[v...]]
    const ts = d.map((p) => p[0])
    const vs = d.map((p) => p[1])
    return [ts, vs]
  }

  onMount(() => {
    plot = new uPlot(opts(), toUplotData(data), el)
    const ro = new ResizeObserver(() => {
      plot?.setSize({ width: el.clientWidth, height })
    })
    ro.observe(el)
    return () => {
      ro.disconnect()
      plot?.destroy()
      plot = null
    }
  })

  $effect(() => {
    if (plot) plot.setData(toUplotData(data))
  })
</script>

<div class="chart" bind:this={el} data-testid={testid}></div>

<style>
  .chart {
    width: 100%;
  }
</style>
