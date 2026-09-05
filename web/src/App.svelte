<script>
  /**
   * App shell (WD7+WD8+WD9): adaptif + auth guard + WS realtime.
   */
  import TopBar from './lib/m3/TopBar.svelte'
  import NavRail from './lib/m3/NavRail.svelte'
  import NavBottom from './lib/m3/NavBottom.svelte'
  import Badge from './lib/m3/Badge.svelte'
  import LoginPage from './pages/LoginPage.svelte'
  import OverviewPage from './pages/OverviewPage.svelte'
  import HostDetailPage from './pages/HostDetailPage.svelte'
  import EventsPage from './pages/EventsPage.svelte'
  import { i18n, t, setLocale, availableLocales } from './lib/i18n.svelte.js'
  import { theme, toggleTheme, applyTheme } from './lib/theme.svelte.js'
  import { route as routeState, navigate } from './lib/router.svelte.js'
  import { checkAuth, setUnauthorizedHandler, logout } from './lib/api.js'
  import { createWsClient } from './lib/ws.js'
  import { hostsStore, applyHello, applySnapshot, applyStatus } from './lib/stores/hosts.svelte.js'
  import { applyWsEvent } from './lib/stores/events.svelte.js'

  const route = $derived(routeState.path)
  let isMobile = $state(window.innerWidth < 900)
  let authed = $state(null) // null = probing
  let wsState = $state('idle')
  let eventsBadge = $state(0) // WE4 mengisi

  applyTheme()

  setUnauthorizedHandler(() => {
    authed = false
    navigate('/login')
  })

  // WD9: WS lifecycle terikat auth
  let ws = null
  function startWs() {
    ws = createWsClient({
      onHello: (data) => applyHello(data),
      onSnapshot: (snap) => applySnapshot(snap),
      onEvent: (ev) => {
        eventsBadge += 1
        applyWsEvent(ev)
        if (document.hidden) {
          document.title = `(${eventsBadge}) hiworld`
        }
      },
      onHostStatus: (st) => applyStatus(st),
      onStateChange: (s) => (wsState = s),
    })
    ws.connect()
  }

  // WD8 fix produksi: setelah login sukses dari LoginPage — authed true
  // dan start WS (sebelumnya authed tetap false → redirect balik login).
  function handleLoginSuccess() {
    authed = true
    if (!ws) startWs()
  }

  // probe sesi saat init (WD-AC-010); sukses → mulai WS
  $effect(() => {
    checkAuth().then((ok) => {
      authed = ok
      if (ok) {
        if (!ws) startWs()
      } else if (route !== '/login') {
        navigate('/login')
      }
    })
    return () => ws?.close()
  })

  // WE4: fokus kembali → reset badge & title
  $effect(() => {
    const reset = () => {
      if (!document.hidden && eventsBadge > 0) {
        eventsBadge = 0
        document.title = 'hiworld monitoring'
      }
    }
    window.addEventListener('focus', reset)
    document.addEventListener('visibilitychange', reset)
    return () => {
      window.removeEventListener('focus', reset)
      document.removeEventListener('visibilitychange', reset)
    }
  })

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

  const showChrome = $derived(authed === true && route !== '/login')
  const wsBadgeColor = $derived(
    wsState === 'connected' ? 'success' : wsState === 'connecting' || wsState === 'reconnecting' ? 'warning' : 'error'
  )
  const wsLabel = $derived(
    wsState === 'connected'
      ? t('common.connected')
      : wsState === 'connecting' || wsState === 'reconnecting'
        ? t('common.reconnecting')
        : t('common.disconnected')
  )

  async function doLogout() {
    ws?.close()
    await logout()
    authed = false
    navigate('/login')
  }
</script>

{#if route === '/login' || authed === false}
  <LoginPage onLogin={handleLoginSuccess} />
{:else if authed === null}
  <main class="boot" data-testid="boot-probe">
    <p>{t('common.loading')}</p>
  </main>
{:else}
  <div class="shell" class:mobile={isMobile}>
    {#if showChrome && !isMobile}
      <aside class="rail">
        <NavRail items={NAV} active={activeNav} onnavigate={navigate} />
      </aside>
    {/if}

    <div class="content">
      <TopBar title={t('overview.title')}>
        <span class="ws-state" data-testid="ws-state" title={wsLabel}>
          <Badge color={wsBadgeColor} pulse={wsState !== 'connected'} />
          <span class="ws-label">{wsLabel}</span>
        </span>
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
        <button data-testid="logout-btn" onclick={doLogout} aria-label="Logout">⎋</button>
      </TopBar>

      <main class="page">
        {#if route === '/'}
          <h1>{t('nav.overview')}</h1>
          <OverviewPage />
        {:else if route.startsWith('/host/')}
          {@const hostId = route.split('/')[2]}
          <HostDetailPage {hostId} />
        {:else if route === '/events'}
          <h1>{t('nav.events')}</h1>
          <EventsPage />
        {:else if route === '/settings'}
          <h1>{t('nav.settings')}</h1>
          <p data-testid="page-settings">settings placeholder (WD9)</p>
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
{/if}

<style>
  .boot {
    min-height: 100vh;
    display: grid;
    place-items: center;
  }
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
    padding: 12px 16px calc(12px + 72px);
  }
  .ws-state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: var(--md-type-label);
    color: var(--md-on-surface-variant);
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
