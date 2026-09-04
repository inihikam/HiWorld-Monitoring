<script>
  /**
   * App shell (WD7, WD-AC-008/009): layout adaptif + hash routing.
   * ≥900px → NavRail kiri; <900px → NavBottom. Login page tanpa nav.
   */
  import TopBar from './lib/m3/TopBar.svelte'
  import NavRail from './lib/m3/NavRail.svelte'
  import NavBottom from './lib/m3/NavBottom.svelte'
  import Badge from './lib/m3/Badge.svelte'
  import { i18n, t, setLocale, availableLocales } from './lib/i18n.svelte.js'
  import { theme, toggleTheme, applyTheme } from './lib/theme.svelte.js'
  import { route as routeState, navigate } from './lib/router.svelte.js'

  const route = $derived(routeState.path)
  let isMobile = $state(window.innerWidth < 900)
  let authed = $state(true) // probe auth menyusul WD8; scaffold asumsi true
  let eventsBadge = $state(0) // WE4 mengisi ini

  applyTheme()


  // breakpoint adaptif via matchMedia (bukan resize spam)
  const mq = window.matchMedia('(max-width: 899px)')
  $effect(() => {
    const fn = (e) => (isMobile = e.matches)
    mq.addEventListener('change', fn)
    return () => mq.removeEventListener('change', fn)
  })

  const NAV = $derived([
    { href: '#/', icon: '▦', label: t('nav.overview') },
    { href: '#/events', icon: '⚡', label: t('nav.events'), badge: eventsBadge || null },
    { href: '#/settings', icon: '⚙', label: t('nav.settings') },
  ])

  const activeNav = $derived(
    NAV.find((n) => route.startsWith(n.href.slice(1) + '/') || n.href === `#${route}`)
      ?.href ?? '#/'
  )

  const showChrome = $derived(authed && route !== '/login')
</script>

<div class="shell" class:mobile={isMobile}>
  {#if showChrome && !isMobile}
    <aside class="rail">
      <NavRail items={NAV} active={activeNav} onnavigate={navigate} />
    </aside>
  {/if}

  <div class="content">
    <TopBar title={t('overview.title')}>
      <select
        data-testid="locale-select"
        value={i18n.locale}
        onchange={(e) => setLocale(e.target.value)}
        aria-label="Language"
      >
        {#each availableLocales() as loc}
          <option value={loc}>{loc.toUpperCase()}</option>
        {/each}
      </select>
      <button
        data-testid="theme-toggle"
        onclick={toggleTheme}
        aria-label="Toggle theme"
        title={theme.current}
      >
        {theme.current === 'dark' ? '☀' : '☾'}
      </button>
    </TopBar>

    <main class="page">
      {#if route === '/'}
        <h1>{t('nav.overview')}</h1>
        <p data-testid="page-overview">overview placeholder (WO3)</p>
      {:else if route.startsWith('/host/')}
        <h1>{t('nav.host_detail')}</h1>
        <p data-testid="page-host">host detail placeholder (WH3): {route}</p>
      {:else if route === '/events'}
        <h1>{t('nav.events')}</h1>
        <p data-testid="page-events">events placeholder (WE2)</p>
      {:else if route === '/settings'}
        <h1>{t('nav.settings')}</h1>
        <p data-testid="page-settings">settings placeholder (WD8)</p>
      {:else if route === '/login'}
        <h1>{t('login.title')}</h1>
        <p data-testid="page-login">login placeholder (WD8)</p>
      {:else}
        <h1>404</h1>
        <p data-testid="page-404">unknown route: {route}</p>
      {/if}
    </main>

    {#if showChrome && isMobile}
      <NavBottom items={NAV} active={activeNav} onnavigate={navigate} />
    {/if}
  </div>
</div>

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }
  .rail {
    flex-shrink: 0;
  }
  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .page {
    flex: 1;
    padding: 16px 24px;
  }
  .shell.mobile .page {
    padding: 12px 16px calc(12px + 72px); /* ruang bottom nav */
  }

  /* TopBar controls */
  select,
  button {
    height: 36px;
    border: 1px solid var(--md-outline);
    background: var(--md-surface-container-high);
    color: var(--md-on-surface);
    border-radius: var(--md-radius-full);
    padding: 0 12px;
    cursor: pointer;
    font: var(--md-type-label);
  }
</style>
