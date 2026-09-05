<script>
  /** LoginPage (WD8, WD-AC-010): form → POST /api/login → redirect. */
  import TextField from '../lib/m3/TextField.svelte'
  import Button from '../lib/m3/Button.svelte'
  import { t } from '../lib/i18n.svelte.js'
  import { login } from '../lib/api.js'
  import { navigate } from '../lib/router.svelte.js'

  let { onLogin = null } = $props()
  let username = $state('')
  let password = $state('')
  let error = $state(null)
  let busy = $state(false)

  async function submit(e) {
    e.preventDefault()
    error = null
    busy = true
    try {
      const ok = await login(username, password)
      if (ok) {
        onLogin?.()
        navigate('/')
      } else {
        error = t('login.error')
      }
    } catch {
      error = t('login.error')
    } finally {
      busy = false
    }
  }
</script>

<div class="login-wrap">
  <form class="login-card" onsubmit={submit} data-testid="login-form">
    <h1>{t('login.title')}</h1>
    <TextField label={t('login.username')} bind:value={username} testid="login-username" />
    <TextField
      label={t('login.password')}
      type="password"
      bind:value={password}
      testid="login-password"
    />
    {#if error}
      <p class="error" data-testid="login-error">{error}</p>
    {/if}
    <Button type="submit" disabled={busy} onclick={submit}>
      {t('login.submit')}
    </Button>
  </form>
</div>

<style>
  .login-wrap {
    min-height: 100vh;
    display: grid;
    place-items: center;
    background: var(--md-surface);
  }
  .login-card {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: min(360px, 90vw);
    background: var(--md-surface-container-low);
    border-radius: var(--md-radius-lg);
    padding: 32px;
    box-shadow: var(--md-elev-2);
  }
  h1 {
    font: var(--md-type-headline);
    color: var(--md-on-surface);
    margin: 0;
    text-align: center;
  }
  .error {
    color: var(--md-error);
    font: var(--md-type-body);
    margin: 0;
  }
</style>
