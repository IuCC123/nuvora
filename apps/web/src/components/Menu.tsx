import { createContext, useContext, useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { CheckIcon } from "./Icon";
import "../styles/dropdown.css";

const CloseMenu = createContext<(refocus: boolean) => void>(() => {});

/**
 * A button that opens a menu of choices. Focus moves to the checked item (or
 * the first), Up and Down move, Home and End jump, a letter jumps to the next
 * item starting with it, Escape closes and returns to the button, Tab closes,
 * and a click anywhere else closes.
 */
export function Menu({
  label,
  trigger,
  triggerClassName = "tool-btn",
  triggerLabel,
  align = "start",
  children,
}: {
  /** Names the menu for assistive tech. */
  label: string;
  trigger: ReactNode;
  triggerClassName?: string;
  /** The button's accessible name when its visible text alone is unclear. */
  triggerLabel?: string;
  align?: "start" | "end";
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();

  useEffect(() => {
    if (!open) return;
    const items = menuItems(menu.current);
    (items.find((i) => i.getAttribute("aria-checked") === "true") ?? items[0])?.focus();
    const outside = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!menu.current?.contains(target) && !button.current?.contains(target)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);

  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const items = menuItems(e.currentTarget);
    const at = items.indexOf(document.activeElement as HTMLElement);
    const focus = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === "ArrowDown") focus(at + 1);
    else if (e.key === "ArrowUp") focus(at - 1);
    else if (e.key === "Home") focus(0);
    else if (e.key === "End") focus(items.length - 1);
    else if (e.key === "Escape") close(true);
    else if (e.key === "Tab") return close(false);
    else if (e.key.length === 1 && /\S/.test(e.key)) {
      const key = e.key.toLowerCase();
      const order = [...items.slice(at + 1), ...items.slice(0, at + 1)];
      order.find((i) => i.textContent?.trim().toLowerCase().startsWith(key))?.focus();
    } else return;
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div className="dropdown">
      <button
        ref={button}
        type="button"
        className={triggerClassName}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={triggerLabel}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {trigger}
      </button>
      {open && (
        <div ref={menu} id={id} role="menu" aria-label={label} className="dropdown-menu" data-align={align} onKeyDown={onKeyDown}>
          <CloseMenu.Provider value={close}>{children}</CloseMenu.Provider>
        </div>
      )}
    </div>
  );
}

function menuItems(menu: HTMLElement | null): HTMLElement[] {
  return [...(menu?.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"])') ?? [])];
}

/** One of a set of exclusive choices. Choosing it closes the menu. */
export function MenuRadio({ checked, onSelect, children, aside }: { checked: boolean; onSelect: () => void; children: ReactNode; aside?: ReactNode }) {
  const close = useContext(CloseMenu);
  return (
    <button
      type="button"
      role="menuitemradio"
      aria-checked={checked}
      className="dropdown-item"
      onClick={() => {
        onSelect();
        close(true);
      }}
    >
      <span className="dropdown-check" aria-hidden>
        {checked && <CheckIcon size={16} />}
      </span>
      <span className="dropdown-text">{children}</span>
      {aside != null && <span className="dropdown-aside">{aside}</span>}
    </button>
  );
}

/** An on/off choice. The menu stays open, so several can be set in a row. */
export function MenuCheckbox({ checked, onToggle, children }: { checked: boolean; onToggle: () => void; children: ReactNode }) {
  return (
    <button type="button" role="menuitemcheckbox" aria-checked={checked} className="dropdown-item" onClick={onToggle}>
      <span className="dropdown-check" aria-hidden>
        {checked && <CheckIcon size={16} />}
      </span>
      <span className="dropdown-text">{children}</span>
    </button>
  );
}

export function MenuGroup({ label, children }: { label: string; children: ReactNode }) {
  const id = useId();
  return (
    <div role="group" aria-labelledby={id} className="dropdown-group">
      <p id={id} className="dropdown-heading">
        {label}
      </p>
      {children}
    </div>
  );
}
