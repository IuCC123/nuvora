import { useEffect, useState } from "react";
import { AuthScreen } from "../components/AuthScreen";
import { FormError, TextField } from "../components/Field";
import { ServerIcon, SpinIcon } from "../components/Icon";
import { desktop, desktopError, type DesktopServer } from "../platform/desktop";
import "../styles/forms.css";
import "../styles/connect.css";

type Bridge = NonNullable<typeof desktop>;

/**
 * The Mac and Windows apps' first page, bundled with the app: it reconnects to
 * the remembered server, or finds one on the network or by address, then
 * hands the window to that server's own web app. `?pick` (Switch Server)
 * skips the reconnect.
 */
export function Connect({ bridge }: { bridge: Bridge }) {
  const picking = new URLSearchParams(location.search).has("pick");
  const [saved, setSaved] = useState<DesktopServer | null | undefined>(undefined);
  const [reconnecting, setReconnecting] = useState(!picking);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void bridge.savedServer().then((server) => {
      setSaved(server);
      if (!server || picking) return setReconnecting(false);
      bridge.reconnect().then(open, (e: unknown) => {
        setError(desktopError(e));
        setReconnecting(false);
      });
    });
  }, [bridge, picking]);

  if (saved === undefined || (reconnecting && saved)) {
    return (
      <AuthScreen>
        <div className="auth-card connect-waiting" aria-busy="true">
          <SpinIcon size={22} className="connect-spin" />
          <h1>{saved ? `Connecting to ${saved.name}` : "Starting"}</h1>
          {saved && <p className="auth-lede">{host(saved.url)}</p>}
        </div>
      </AuthScreen>
    );
  }
  return <Picker bridge={bridge} saved={saved} initialError={error} />;
}

function Picker({ bridge, saved, initialError }: { bridge: Bridge; saved: DesktopServer | null; initialError: string | null }) {
  const found = useNetworkServers(bridge);
  const [address, setAddress] = useState(saved ? host(saved.url) : "");
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState(initialError);

  const connect = (typed: string) => {
    setPending(typed);
    setError(null);
    bridge.connect(typed).then(open, (e: unknown) => {
      setError(desktopError(e));
      setPending(null);
    });
  };

  const servers = saved && !found.some((s) => s.id && s.id === saved.id) ? [saved, ...found] : found;
  return (
    <AuthScreen>
      <div className="auth-card">
        <h1>Connect to Nuvora</h1>
        <p className="auth-lede">Pick your server, or type the address you open it at in a browser.</p>
        <section className="connect-found" aria-labelledby="connect-found">
          <h2 id="connect-found">On your network</h2>
          {servers.length === 0 ? (
            <p className="connect-looking">
              <SpinIcon size={14} className="connect-spin" />
              Looking for servers…
            </p>
          ) : (
            <ul role="list">
              {servers.map((server) => (
                <li key={server.url}>
                  <button type="button" className="connect-server" disabled={pending !== null} aria-busy={pending === server.url} onClick={() => connect(server.url)}>
                    <ServerIcon size={18} />
                    <span className="connect-server-name">{server.name}</span>
                    <span className="connect-server-host">{host(server.url)}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
        <form
          className="form"
          noValidate
          onSubmit={(e) => {
            e.preventDefault();
            if (!address.trim()) return setError("Type your server's address, like 192.168.1.20.");
            connect(address);
          }}
        >
          <TextField
            label="Server address"
            placeholder="192.168.1.20"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            autoFocus={servers.length === 0}
            hint="Port 8640 is assumed when you leave it out."
            value={address}
            onChange={(e) => setAddress(e.target.value)}
          />
          <FormError message={error} />
          <button type="submit" className="btn btn-primary btn-block" disabled={pending !== null} aria-busy={pending === address}>
            {pending === null ? "Connect" : "Connecting…"}
          </button>
        </form>
      </div>
      <p className="auth-foot">
        No server yet? <a href="https://github.com/IuCC123/nuvora#set-up-your-server">Set one up</a> on a computer or NAS at home.
      </p>
    </AuthScreen>
  );
}

/** Servers announcing themselves on the local network, refreshed every second. */
function useNetworkServers(bridge: Bridge): DesktopServer[] {
  const [servers, setServers] = useState<DesktopServer[]>([]);
  useEffect(() => {
    const read = () => void bridge.discoveredServers().then(setServers);
    read();
    const timer = setInterval(read, 1000);
    return () => clearInterval(timer);
  }, [bridge]);
  return servers;
}

function open(server: DesktopServer) {
  location.replace(server.url);
}

function host(url: string): string {
  return url.replace(/^https?:\/\//, "");
}
