<script>
  // Scaffold minimal — shell lengkap menyusul di WD7.
  // Svelte 5 runes; probe auth + render placeholder.
  let ready = $state(false)

  $effect(() => {
    // probe sesi: 401 → (nantinya) redirect login; v0 scaffold hanya tampil
    fetch('/api/hosts', { credentials: 'same-origin' })
      .then(() => { ready = true })
      .catch(() => { ready = true })
  })
</script>

<main class="scaffold">
  <h1>hiworld monitoring</h1>
  <p data-testid="scaffold-status">{ready ? 'api reachable' : 'connecting…'}</p>
</main>

<style>
  .scaffold {
    display: grid;
    place-items: center;
    min-height: 100vh;
  }
</style>
