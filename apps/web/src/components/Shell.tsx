import { Outlet, useRouterState } from "@tanstack/react-router";
import { SettingsRail, TabBar } from "./NavRail";
import { TopBar } from "./TopBar";
import { UpdateNotice } from "./Updates";
import "../styles/shell.css";

/**
 * A floating top bar over full-bleed screens, a tab bar on phones, and in
 * Settings a section rail beside the page. Screens own their top spacing,
 * so a hero can run under the bar.
 */
export function Shell() {
  const settings = useRouterState({ select: (s) => s.location.pathname.startsWith("/settings") });
  return (
    <div className="app">
      <a href="#content" className="skip-link">
        Skip to content
      </a>
      <TopBar />
      <main id="content" className="content" tabIndex={-1}>
        {settings ? (
          <div className="settings-shell">
            <SettingsRail />
            <div className="settings-main">
              <Outlet />
            </div>
          </div>
        ) : (
          <Outlet />
        )}
      </main>
      <TabBar />
      <UpdateNotice />
    </div>
  );
}
