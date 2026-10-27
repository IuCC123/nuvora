import { Outlet } from "@tanstack/react-router";
import { NavRail } from "./NavRail";
import { TopBar } from "./TopBar";
import { UpdateNotice } from "./Updates";
import "../styles/shell.css";

export function Shell() {
  return (
    <div className="app">
      <a href="#content" className="skip-link">
        Skip to content
      </a>
      <TopBar />
      <NavRail />
      <main id="content" className="content" tabIndex={-1}>
        <Outlet />
      </main>
      <UpdateNotice />
    </div>
  );
}
