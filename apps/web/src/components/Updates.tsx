import type { ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { queryKeys, type UpdateStatus } from "@nuvora/core";
import { client } from "../nuvora";
import {
  useDesktopApp,
  useDesktopUpdate,
  useDismissed,
  useInstallDesktopUpdate,
  useInstallServerUpdate,
  useServerUpdate,
  useServerUpdated,
} from "../lib/updates";
import { AlertIcon, CheckIcon, SpinIcon } from "./Icon";
import "../styles/forms.css";
import "../styles/updates.css";

/** Settings › General: this server's version, the newest, and the button that installs it. */
export function ServerUpdates() {
  const qc = useQueryClient();
  const status = useServerUpdate();
  const { install, start } = useInstallServerUpdate();
  const checkAgain = async () => {
    qc.setQueryData(queryKeys.adminUpdate, await client.admin.update(true).catch(() => status.data));
  };
  return (
    <section className="settings-section" id="updates" aria-labelledby="updates-title">
      <h2 id="updates-title">Updates</h2>
      <div className="updates" aria-live="polite">
        {status.isPending ? (
          <p className="update-line">
            <SpinIcon className="update-spin" /> Checking for updates…
          </p>
        ) : !status.data ? (
          <p className="update-line">Couldn't check for updates.</p>
        ) : install.step === "downloading" || install.step === "restarting" ? (
          <>
            <p className="update-line">
              <SpinIcon className="update-spin" />
              {install.step === "downloading" ? `Downloading Nuvora ${install.version}…` : `Restarting as Nuvora ${install.version}…`}
            </p>
            <p className="settings-note">This page reloads by itself when the server is back, usually within a minute.</p>
          </>
        ) : (
          <ServerStatus status={status.data} onInstall={(v) => void start(v)} onCheck={() => void checkAgain()} />
        )}
        {install.step === "failed" && (
          <p className="update-error" role="alert">
            <AlertIcon size={16} />
            {install.message}
          </p>
        )}
      </div>
    </section>
  );
}

function ServerStatus({ status, onInstall, onCheck }: { status: UpdateStatus; onInstall: (version: string) => void; onCheck: () => void }) {
  if (!status.checks) {
    return (
      <>
        <p className="update-line">Nuvora {status.current}</p>
        <p className="settings-note">Update checks are turned off on this server (NUVORA_UPDATES=false).</p>
      </>
    );
  }
  if (status.available && status.latest) {
    return (
      <>
        <p className="update-line update-line-strong">Nuvora {status.latest} is available</p>
        <p className="settings-note">
          This server runs {status.current}.{" "}
          {status.release_url && (
            <a href={status.release_url} target="_blank" rel="noreferrer">
              See what's new
            </a>
          )}
        </p>
        {status.can_install ? (
          <>
            <div className="form-actions">
              <button type="button" className="btn btn-primary" onClick={() => onInstall(status.latest!)}>
                Update and restart
              </button>
            </div>
            <p className="settings-note">Nuvora restarts once to finish. Anything playing pauses for a few seconds.</p>
          </>
        ) : (
          <p className="settings-note">
            This server can't update itself. Update it the way you installed it: run the installer again, or run
            <code> docker compose pull && docker compose up -d </code>
            next to its compose.yaml.
          </p>
        )}
      </>
    );
  }
  return (
    <>
      <p className="update-line">
        {status.latest ? <CheckIcon className="update-ok" /> : null}
        {status.latest ? `Nuvora ${status.current} is up to date` : `Nuvora ${status.current}`}
      </p>
      {status.error && <p className="settings-note">{status.error}</p>}
      <div className="form-actions">
        <button type="button" className="btn btn-secondary btn-small" onClick={onCheck}>
          Check again
        </button>
      </div>
    </>
  );
}

/** Settings › Account in the Mac and Windows apps: this app's version and its update. */
export function AppUpdates() {
  const app = useDesktopApp();
  const update = useDesktopUpdate();
  const install = useInstallDesktopUpdate();
  if (!app.data) return null;
  const available = update.data;
  return (
    <section className="settings-section" aria-labelledby="this-app">
      <h2 id="this-app">This app</h2>
      <div className="updates" aria-live="polite">
        {install.isPending ? (
          <p className="update-line">
            <SpinIcon className="update-spin" />
            Updating to {available?.version}. The app restarts by itself…
          </p>
        ) : update.isFetching ? (
          <p className="update-line">
            <SpinIcon className="update-spin" /> Checking for updates…
          </p>
        ) : available ? (
          <>
            <p className="update-line update-line-strong">
              {app.data.name} {available.version} is available
            </p>
            <p className="settings-note">You have {app.data.version}.</p>
            <div className="form-actions">
              <button type="button" className="btn btn-primary" onClick={() => install.mutate()}>
                Update and restart
              </button>
            </div>
          </>
        ) : (
          <>
            <p className="update-line">
              {update.isSuccess && <CheckIcon className="update-ok" />}
              {app.data.name} {app.data.version}
              {update.isSuccess ? " is up to date" : ""}
            </p>
            {update.isError && <p className="settings-note">Couldn't check for updates.</p>}
            <div className="form-actions">
              <button type="button" className="btn btn-secondary btn-small" onClick={() => void update.refetch()}>
                Check again
              </button>
            </div>
          </>
        )}
        {install.message && (
          <p className="update-error" role="alert">
            <AlertIcon size={16} />
            {install.message}
          </p>
        )}
      </div>
    </section>
  );
}

/**
 * One quiet card in the corner when something is newer: this page (the server
 * was updated under it), this Mac or Windows app, or, for admins, the server.
 */
export function UpdateNotice() {
  const updated = useServerUpdated();
  const app = useDesktopApp();
  const appUpdate = useDesktopUpdate();
  const installApp = useInstallDesktopUpdate();
  const server = useServerUpdate();
  const appLater = useDismissed("app");
  const serverLater = useDismissed("server");

  if (updated) {
    return (
      <Card title="Nuvora was updated" text={`Reload to use version ${updated}.`}>
        <button type="button" className="btn btn-primary btn-small" onClick={() => location.reload()}>
          Reload
        </button>
      </Card>
    );
  }
  const appVersion = appUpdate.data?.version;
  if (app.data && appVersion && !appLater.isDismissed(appVersion)) {
    return (
      <Card
        title={`${app.data.name} ${appVersion} is available`}
        text={installApp.message ?? (installApp.isPending ? "Updating. The app restarts by itself…" : `You have ${app.data.version}.`)}
      >
        <button
          type="button"
          className="btn btn-primary btn-small"
          onClick={() => installApp.mutate()}
          disabled={installApp.isPending}
          aria-busy={installApp.isPending}
        >
          Update and restart
        </button>
        <button type="button" className="btn btn-quiet btn-small" onClick={() => appLater.dismiss(appVersion)} disabled={installApp.isPending}>
          Later
        </button>
      </Card>
    );
  }
  const latest = server.data?.available ? server.data.latest : null;
  if (latest && !serverLater.isDismissed(latest)) {
    return (
      <Card title={`Nuvora ${latest} is available`} text={`This server runs ${server.data!.current}.`}>
        <Link to="/settings/general" hash="updates" className="btn btn-primary btn-small" onClick={() => serverLater.dismiss(latest)}>
          See update
        </Link>
        <button type="button" className="btn btn-quiet btn-small" onClick={() => serverLater.dismiss(latest)}>
          Later
        </button>
      </Card>
    );
  }
  return null;
}

function Card({ title, text, children }: { title: string; text: string; children: ReactNode }) {
  return (
    <aside className="update-card" role="status" aria-label={title}>
      <p className="update-card-title">{title}</p>
      <p className="update-card-text">{text}</p>
      <div className="update-card-actions">{children}</div>
    </aside>
  );
}
