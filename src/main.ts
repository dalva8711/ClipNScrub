import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './style.css';

type Detector = 'aws' | 'jwt' | 'email' | 'ip' | 'api_key' | 'database_url' | 'private_key' | 'customer_id';
interface Settings { enabled: boolean; shortcut: string; detectors: Record<Detector, boolean>; customer_patterns: string[] }
interface Report { status: string; message: string; counts: Record<string, number> }
interface Snapshot { settings: Settings; report: Report }

const detectors: [Detector, string, string][] = [
  ['aws', 'AWS keys', 'Access keys & labeled secrets'],
  ['jwt', 'JWTs', 'Signed authentication tokens'],
  ['email', 'Email addresses', 'Consistent numbered placeholders'],
  ['ip', 'IP addresses', 'IPv4 & IPv6 addresses'],
  ['api_key', 'API keys & secrets', 'Provider tokens & labeled credentials'],
  ['database_url', 'Database URLs', 'Entire connection strings'],
  ['private_key', 'Private keys', 'Multiline PEM & OpenSSH blocks'],
  ['customer_id', 'Customer IDs', 'Labeled IDs & your custom patterns'],
];

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <main>
    <header><div class="brand"><span class="logo" aria-hidden="true">⌁</span><span>ClipNScrub<span class="version"> / 0.1</span></span></div><span class="local"><i></i> ENTIRELY OFFLINE</span></header>
    <section class="intro"><p class="eyebrow">SHARE THE CONTEXT. KEEP THE SECRETS.</p><h1>A cleaner clipboard.</h1><p class="subtitle">Sanitize sensitive text before it leaves your hands.</p></section>
    <section class="control" aria-label="Clipboard controls"><div><span id="state-label" class="state-label">Connecting…</span><p id="state-description">Your clipboard is only read when you ask.</p></div><button id="toggle" class="switch" role="switch" aria-checked="false" aria-label="Enable ClipNScrub" disabled><span></span></button></section>
    <section class="workflow" aria-label="How to use ClipNScrub"><div><b>01</b><span>Copy your text</span></div><span class="arrow">→</span><div><b>02</b><span>Sanitize <kbd id="shortcut-hint">⌘ ⇧ S</kbd></span></div><span class="arrow">→</span><div><b>03</b><span>Paste anywhere</span></div></section>
    <div class="action-row"><button id="sanitize" class="primary" disabled>Sanitize clipboard <span>↗</span></button><span>Text stays on this device.</span></div>
    <div id="status" class="status" role="status" aria-live="polite">Connecting to the desktop app…</div>
    <form id="settings-form">
      <section class="section"><div class="section-heading"><h2>What gets scrubbed</h2><span>Balanced detection</span></div><div class="detectors">${detectors.map(([id, label, description]) => `<label class="detector"><input type="checkbox" name="${id}" disabled><span><strong>${label}</strong><small>${description}</small></span></label>`).join('')}</div></section>
      <section class="section preferences"><div><h2>Keyboard shortcut</h2><p class="help">Use CommandOrControl for ⌘ on Mac and Ctrl on Windows.</p><label class="sr-only" for="shortcut">Keyboard shortcut</label><input id="shortcut" type="text" spellcheck="false" autocomplete="off" placeholder="CommandOrControl+Shift+S" maxlength="100" disabled></div><div><label class="field-label" for="patterns">Custom customer-ID patterns <span>Optional</span></label><p class="help" id="pattern-help">One Rust regex per line. The entire match becomes a customer-ID placeholder. Maximum 20 patterns.</p><textarea id="patterns" rows="3" spellcheck="false" aria-describedby="pattern-help" placeholder="&#92;bcus_[A-Za-z0-9]+&#92;b" disabled></textarea></div></section>
      <div class="save-row"><span id="save-status" role="status" aria-live="polite">Preferences are saved on this device.</span><button class="secondary" id="save" type="submit" disabled>Save settings</button></div>
    </form>
    <details class="example"><summary>See an example <span>+</span></summary><pre>DATABASE_URL=&lt;REDACTED&gt;
API_KEY=&lt;REDACTED&gt;
USER_EMAIL=&lt;EMAIL_1&gt;</pre><p>Secrets are removed. Repeated emails, IPs, and customer IDs keep consistent placeholders within each sanitization.</p></details>
    <footer><span class="footer-mark">⌁</span><p>No accounts. No tracking. No clipboard history.<br><span>Detection is best-effort. Review before sharing. Other clipboard managers may retain originals.</span></p><span class="license">FREE & OPEN SOURCE</span></footer>
  </main>`;

const $ = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector)!;
let current: Settings | undefined;
let busy = false;

function showReport(report: Report) {
  const labels: Record<string, [string, string]> = { secrets: ['secret', 'secrets'], emails: ['email', 'emails'], ip_addresses: ['IP address', 'IP addresses'], customer_ids: ['customer ID', 'customer IDs'] };
  const counts = Object.entries(report.counts).map(([key, value]) => `${value} ${labels[key]?.[value === 1 ? 0 : 1] ?? key}`).join(' · ');
  $('#status').textContent = report.message + (counts ? ` ${counts}` : '');
  $('#status').classList.toggle('error', report.status === 'error');
}
function showEnabled(settings: Settings) {
  $('#state-label').textContent = settings.enabled ? 'Ready when you are' : 'ClipNScrub is paused';
  $('#state-description').textContent = settings.enabled ? 'Your clipboard is only read when you ask.' : 'Enable to use the shortcut or sanitize action.';
  $('#toggle').setAttribute('aria-checked', String(settings.enabled));
  $<HTMLButtonElement>('#sanitize').disabled = !settings.enabled || busy;
  $('#shortcut-hint').textContent = settings.shortcut.replace('CommandOrControl', navigator.userAgent.includes('Mac') ? '⌘' : 'Ctrl').replaceAll('+', ' + ');
}
function populate(settings: Settings) {
  current = settings;
  showEnabled(settings);
  $<HTMLInputElement>('#shortcut').value = settings.shortcut;
  $<HTMLTextAreaElement>('#patterns').value = settings.customer_patterns.join('\n');
  for (const [id] of detectors) $<HTMLInputElement>(`input[name="${id}"]`).checked = settings.detectors[id];
}
function message(error: unknown): string { return typeof error === 'string' ? error : 'The desktop app could not complete this action. Try again.'; }
function lock(value: boolean) {
  busy = value;
  document.querySelectorAll<HTMLInputElement | HTMLButtonElement | HTMLTextAreaElement>('input, button, textarea').forEach(el => { el.disabled = value; });
  if (current) showEnabled(current);
}

$('#toggle').addEventListener('click', async () => {
  if (!current || busy) return;
  lock(true);
  try { current = await invoke<Settings>('set_enabled', { enabled: !current.enabled }); showEnabled(current); }
  catch (error) { showReport({ status: 'error', message: message(error), counts: {} }); }
  finally { lock(false); }
});
$('#sanitize').addEventListener('click', async () => {
  if (!current || busy) return;
  lock(true);
  try { showReport(await invoke<Report>('sanitize_clipboard')); }
  catch (error) { showReport({ status: 'error', message: message(error), counts: {} }); }
  finally { lock(false); }
});
$('#settings-form').addEventListener('submit', async event => {
  event.preventDefault();
  if (!current || busy) return;
  const settings: Settings = {
    enabled: current.enabled,
    shortcut: $<HTMLInputElement>('#shortcut').value.trim(),
    customer_patterns: $<HTMLTextAreaElement>('#patterns').value.split('\n').map(p => p.trim()).filter(Boolean),
    detectors: Object.fromEntries(detectors.map(([id]) => [id, $<HTMLInputElement>(`input[name="${id}"]`).checked])) as Record<Detector, boolean>,
  };
  lock(true);
  try { populate(await invoke<Settings>('save_settings', { settings })); $('#save-status').textContent = 'Settings saved.'; $('#save-status').classList.remove('error'); }
  catch (error) { $('#save-status').textContent = message(error); $('#save-status').classList.add('error'); }
  finally { lock(false); }
});

async function start() {
  try {
    await listen<Report>('status', event => showReport(event.payload));
    await listen<Settings>('settings-changed', event => {
      // Tray toggles must not overwrite an unsaved detector/shortcut edit.
      if (current) { current.enabled = event.payload.enabled; showEnabled(current); }
    });
    const snapshot = await invoke<Snapshot>('get_snapshot');
    populate(snapshot.settings); showReport(snapshot.report); lock(false);
  } catch {
    showReport({ status: 'error', message: 'Open ClipNScrub as a desktop app to use clipboard controls. Run npm run tauri dev during development.', counts: {} });
  }
}
void start();
