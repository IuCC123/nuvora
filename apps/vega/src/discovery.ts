import { fetch as netInfo } from '@amazon-devices/keplerscript-netmgr-lib';
import type { Server } from './server';

/** Nuvora's port. */
const PORT = 8640;
const TIMEOUT_MS = 1_500;
const AT_ONCE = 48;

/** The TV's IPv4 address, or null when it has none (or won't say). */
async function ownAddress(): Promise<string | null> {
  try {
    const state = await netInfo();
    const details = state.details as { ipAddress?: string | null } | null;
    const ip = details?.ipAddress ?? null;
    return ip && /^\d+\.\d+\.\d+\.\d+$/.test(ip) ? ip : null;
  } catch {
    return null;
  }
}

/** Asks `url` whether it is a Nuvora server, giving up after a moment. */
async function probe(url: string, signal: { aborted: boolean }): Promise<Server | null> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(`${url}/api/auth/state`, { signal: controller.signal });
    if (!res.ok || signal.aborted) return null;
    const state = (await res.json()) as { server_name?: string; server_id?: string; setup_required?: boolean };
    if (typeof state.server_name !== 'string' || state.setup_required) return null;
    return { url, name: state.server_name, id: state.server_id || null };
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Finds Nuvora servers on the TV's network. Vega has no mDNS for apps, so
 * this asks every address on the TV's /24 for Nuvora's port, a few dozen at
 * a time; each server is reported as it answers. On the virtual device the
 * computer running it (10.0.2.2) is asked too.
 */
export async function discoverServers(onFound: (server: Server) => void, signal: { aborted: boolean }): Promise<void> {
  const ip = await ownAddress();
  if (!ip || signal.aborted) return;
  const prefix = ip.split('.').slice(0, 3).join('.');
  const hosts = Array.from({ length: 254 }, (_, i) => `${prefix}.${i + 1}`).filter((h) => h !== ip);
  // The virtual device's computer first: it's usually the server there.
  if (prefix === '10.0.2') hosts.sort((a, b) => (a === '10.0.2.2' ? -1 : b === '10.0.2.2' ? 1 : 0));
  let next = 0;
  const worker = async () => {
    while (!signal.aborted && next < hosts.length) {
      const host = hosts[next++]!;
      const found = await probe(`http://${host}:${PORT}`, signal);
      if (found && !signal.aborted) onFound(found);
    }
  };
  await Promise.all(Array.from({ length: AT_ONCE }, worker));
}
