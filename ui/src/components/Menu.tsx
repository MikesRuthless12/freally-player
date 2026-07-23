import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/**
 * A small popover menu — a trigger and an upward-opening list that closes on outside click or
 * Escape.
 *
 * The list is rendered through a portal to `document.body`, positioned `fixed` above the
 * trigger. The control strip collapses via `overflow-hidden`, which would otherwise clip a menu
 * that opens above it; the portal escapes that clip (and any stacking context).
 *
 * Lives in its own module (not `ControlBar`) because the control bar, the audio menu, and the
 * subtitle menu all consume it — keeping it here avoids an import cycle between them.
 */
export function Menu({
  label,
  icon,
  title,
  disabled = false,
  align = "start",
  className = "",
  panelClassName = "min-w-32",
  children,
}: {
  label?: string;
  icon?: React.ReactNode;
  title: string;
  disabled?: boolean;
  align?: "start" | "end";
  className?: string;
  /** Extra classes for the popover panel — e.g. a wider panel for the subtitle controls. */
  panelClassName?: string;
  children: (close: () => void) => React.ReactNode;
}) {
  const [coords, setCoords] = useState<{
    bottom: number;
    left?: number;
    right?: number;
  } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const open = coords !== null;

  const openMenu = () => {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    // Anchor the list's bottom just above the trigger; align its near edge to the trigger's.
    const gap = 6;
    setCoords(
      align === "end"
        ? { bottom: window.innerHeight - rect.top + gap, right: window.innerWidth - rect.right }
        : { bottom: window.innerHeight - rect.top + gap, left: rect.left },
    );
  };

  useEffect(() => {
    if (!open) return;
    const onDocPointer = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!triggerRef.current?.contains(target) && !menuRef.current?.contains(target)) {
        setCoords(null);
      }
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setCoords(null);
    };
    document.addEventListener("pointerdown", onDocPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDocPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => (open ? setCoords(null) : openMenu())}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={title}
        title={title}
        disabled={disabled}
        className={`grid h-8 min-w-8 place-items-center rounded-md px-1.5 text-havoc-muted transition-colors hover:bg-havoc-surface hover:text-havoc-text disabled:cursor-not-allowed disabled:opacity-40 ${className}`}
      >
        {icon ?? label}
      </button>
      {open &&
        createPortal(
          <div
            ref={menuRef}
            role="menu"
            aria-label={title}
            style={{
              position: "fixed",
              bottom: coords.bottom,
              left: coords.left,
              right: coords.right,
            }}
            className={`z-50 max-h-[70vh] overflow-auto rounded-lg border border-havoc-border bg-havoc-panel p-1 shadow-xl ${panelClassName}`}
          >
            {children(() => setCoords(null))}
          </div>,
          document.body,
        )}
    </>
  );
}

export function MenuItem({
  selected = false,
  onSelect,
  children,
}: {
  selected?: boolean;
  onSelect: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onSelect}
      className={`flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-start text-xs transition-colors ${
        selected
          ? "bg-havoc-accent/20 font-semibold text-havoc-text"
          : "text-havoc-muted hover:bg-havoc-surface hover:text-havoc-text"
      }`}
    >
      {children}
    </button>
  );
}
