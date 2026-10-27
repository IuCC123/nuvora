import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Link } from "@tanstack/react-router";
import { initials } from "@nuvora/core";
import { useSignOut, useUser } from "../lib/session";
import { desktop } from "../platform/desktop";
import { ServerIcon, SettingsIcon, SignOutIcon, UserIcon } from "./Icon";

/**
 * The initials button at the right of the top bar and its menu. Up and Down
 * move between items, Escape closes and returns focus to the button, and a
 * click anywhere else closes it.
 */
export function AccountMenu() {
  const user = useUser();
  const signOut = useSignOut();
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    const outside = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!menu.current?.contains(target) && !button.current?.contains(target)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);

  if (!user) return null;

  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const items = [...e.currentTarget.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const focus = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === "ArrowDown") focus(at + 1);
    else if (e.key === "ArrowUp") focus(at - 1);
    else if (e.key === "Home") focus(0);
    else if (e.key === "End") focus(items.length - 1);
    else if (e.key === "Escape") close(true);
    else if (e.key === "Tab") return close(false);
    else return;
    e.preventDefault();
  };

  return (
    <div className="account">
      <button
        ref={button}
        type="button"
        className="avatar avatar-button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={`Account: ${user.display_name}`}
        onClick={() => setOpen((o) => !o)}
      >
        {initials(user.display_name)}
      </button>
      {open && (
        <div className="menu" id={menuId} role="menu" aria-label="Account" ref={menu} onKeyDown={onKeyDown}>
          <div className="menu-head">
            <span className="avatar avatar-large" aria-hidden>
              {initials(user.display_name)}
            </span>
            <div className="menu-who">
              <p className="menu-name">{user.display_name}</p>
              <p className="menu-handle">
                {user.username} · {user.role === "admin" ? "Admin" : "Member"}
              </p>
            </div>
          </div>
          {user.role === "admin" ? (
            <Link to="/settings/general" role="menuitem" className="menu-item" onClick={() => close(false)}>
              <SettingsIcon size={18} />
              Settings
            </Link>
          ) : (
            <Link to="/settings/account" role="menuitem" className="menu-item" onClick={() => close(false)}>
              <UserIcon size={18} />
              Account
            </Link>
          )}
          {desktop && (
            <button type="button" role="menuitem" className="menu-item" onClick={() => void desktop?.switchServer()}>
              <ServerIcon size={18} />
              Switch server
            </button>
          )}
          <button
            type="button"
            role="menuitem"
            className="menu-item"
            onClick={() => signOut.mutate()}
            disabled={signOut.isPending}
            aria-busy={signOut.isPending}
          >
            <SignOutIcon />
            Sign out
          </button>
          {signOut.isError && (
            <p className="menu-error" role="alert">
              Couldn't sign out. Try again.
            </p>
          )}
        </div>
      )}
    </div>
  );
}
