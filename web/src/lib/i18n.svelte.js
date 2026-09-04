/**
 * i18n sederhana (WD5, WD-AC-006) — ADR WD-3: dict buatan, 2 bahasa.
 * Default EN; persist localStorage 'hiworld-locale'.
 * Struktur: t('nav.overview') → string; fallback: key itu sendiri.
 */

const STORAGE_KEY = 'hiworld-locale'

const DICTS = {
  en: {
    // nav
    'nav.overview': 'Overview',
    'nav.events': 'Events',
    'nav.settings': 'Settings',
    'nav.host_detail': 'Host Detail',
    // common
    'common.loading': 'Loading…',
    'common.retry': 'Retry',
    'common.close': 'Close',
    'common.back': 'Back',
    'common.online': 'Online',
    'common.offline': 'Offline',
    'common.stale': 'Stale',
    'common.waiting_first_sample': 'Waiting first sample…',
    'common.connected': 'Connected',
    'common.reconnecting': 'Reconnecting…',
    'common.disconnected': 'Disconnected',
    // login
    'login.title': 'Sign in',
    'login.username': 'Username',
    'login.password': 'Password',
    'login.submit': 'Sign in',
    'login.error': 'Invalid username or password',
    // overview
    'overview.title': 'Overview',
    'overview.hosts_online': '{online} of {total} hosts online',
    'overview.avg_cpu': 'Avg CPU',
    'overview.avg_ram': 'Avg RAM',
    'overview.setup_agent': 'Setup agent',
    'overview.setup_title': 'Add a new host',
    'overview.setup_body':
      'Run the hiworld-agent binary on the target server and point it to this collector URL. See the README for details.',
    // host detail
    'host.cpu': 'CPU',
    'host.ram': 'RAM',
    'host.disk': 'Disk',
    'host.load_1m': 'Load (1m)',
    'host.processes': 'Processes',
    'host.processes_all': 'Show all',
    'host.processes_top15': 'Show top 15 only',
    'host.process_empty': 'No process data',
    'host.net_rx': 'Total RX',
    'host.net_tx': 'Total TX',
    'host.as_of': 'as of {time}',
    'host.range_15m': '15m',
    'host.range_1h': '1h',
    'host.range_6h': '6h',
    'host.range_24h': '24h',
    'col.pid': 'PID',
    'col.name': 'Name',
    'col.unit': 'Unit',
    'col.cpu': 'CPU %',
    'col.ram': 'RAM %',
    'col.rss': 'RSS',
    // events
    'events.title': 'Events',
    'events.empty': 'No events',
    'events.filter_host': 'All hosts',
    'events.load_more': 'Load more',
    'events.today': 'Today',
    'events.yesterday': 'Yesterday',
    'events.see_host': 'View host',
    'sev.info': 'info',
    'sev.warning': 'warning',
    'sev.critical': 'critical',
    'kind.spike_cpu': 'CPU spike',
    'kind.spike_mem': 'Memory spike',
    'kind.disk_almost_full': 'Disk almost full',
    'kind.agent_down': 'Agent down',
    'kind.agent_up': 'Agent up',
    // settings
    'settings.title': 'Settings',
    'settings.change_password': 'Change password',
    'settings.old_password': 'Current password',
    'settings.new_password': 'New password',
    'settings.confirm_password': 'Confirm new password',
    'settings.save': 'Save',
    'settings.saved': 'Password changed',
    'settings.mismatch': 'Passwords do not match',
    'settings.logout': 'Sign out',
  },
  id: {
    'nav.overview': 'Ringkasan',
    'nav.events': 'Event',
    'nav.settings': 'Pengaturan',
    'nav.host_detail': 'Detail Host',
    'common.loading': 'Memuat…',
    'common.retry': 'Coba lagi',
    'common.close': 'Tutup',
    'common.back': 'Kembali',
    'common.online': 'Online',
    'common.offline': 'Offline',
    'common.stale': 'Usang',
    'common.waiting_first_sample': 'Menunggu sampel pertama…',
    'common.connected': 'Terhubung',
    'common.reconnecting': 'Menyambung ulang…',
    'common.disconnected': 'Terputus',
    'login.title': 'Masuk',
    'login.username': 'Nama pengguna',
    'login.password': 'Kata sandi',
    'login.submit': 'Masuk',
    'login.error': 'Nama pengguna atau kata sandi salah',
    'overview.title': 'Ringkasan',
    'overview.hosts_online': '{online} dari {total} host online',
    'overview.avg_cpu': 'Rata-rata CPU',
    'overview.avg_ram': 'Rata-rata RAM',
    'overview.setup_agent': 'Setup agent',
    'overview.setup_title': 'Tambah host baru',
    'overview.setup_body':
      'Jalankan binari hiworld-agent di server tujuan dan arahkan ke URL collector ini. Lihat README untuk detailnya.',
    'host.cpu': 'CPU',
    'host.ram': 'RAM',
    'host.disk': 'Disk',
    'host.load_1m': 'Load (1m)',
    'host.processes': 'Proses',
    'host.processes_all': 'Tampilkan semua',
    'host.processes_top15': 'Tampilkan 15 teratas',
    'host.process_empty': 'Tidak ada data proses',
    'host.net_rx': 'Total RX',
    'host.net_tx': 'Total TX',
    'host.as_of': 'per {time}',
    'host.range_15m': '15m',
    'host.range_1h': '1j',
    'host.range_6h': '6j',
    'host.range_24h': '24j',
    'col.pid': 'PID',
    'col.name': 'Nama',
    'col.unit': 'Unit',
    'col.cpu': 'CPU %',
    'col.ram': 'RAM %',
    'col.rss': 'RSS',
    'events.title': 'Event',
    'events.empty': 'Belum ada event',
    'events.filter_host': 'Semua host',
    'events.load_more': 'Muat lagi',
    'events.today': 'Hari ini',
    'events.yesterday': 'Kemarin',
    'events.see_host': 'Lihat host',
    'sev.info': 'info',
    'sev.warning': 'peringatan',
    'sev.critical': 'kritis',
    'kind.spike_cpu': 'Lonjakan CPU',
    'kind.spike_mem': 'Lonjakan memori',
    'kind.disk_almost_full': 'Disk hampir penuh',
    'kind.agent_down': 'Agent mati',
    'kind.agent_up': 'Agent hidup',
    'settings.title': 'Pengaturan',
    'settings.change_password': 'Ganti kata sandi',
    'settings.old_password': 'Kata sandi saat ini',
    'settings.new_password': 'Kata sandi baru',
    'settings.confirm_password': 'Konfirmasi kata sandi baru',
    'settings.save': 'Simpan',
    'settings.saved': 'Kata sandi diganti',
    'settings.mismatch': 'Kata sandi tidak cocok',
    'settings.logout': 'Keluar',
  },
}

function detectInitial() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved === 'en' || saved === 'id') return saved
  } catch {
    /* non-fatal */
  }
  return 'en' // ADR: default English
}

export const i18n = $state({ locale: detectInitial() })

/**
 * Terjemahkan key; {placeholder} diisi dari params.
 * Fallback: bila key tak ada di locale aktif, coba EN; tetap tak ada → key.
 */
export function t(key, params = {}) {
  let str = DICTS[i18n.locale]?.[key] ?? DICTS.en[key] ?? key
  for (const [k, v] of Object.entries(params)) {
    str = str.replaceAll(`{${k}}`, String(v))
  }
  return str
}

export function setLocale(locale) {
  if (!DICTS[locale]) return
  i18n.locale = locale
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    /* non-fatal */
  }
}

export function availableLocales() {
  return Object.keys(DICTS)
}
