import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// jsdom has no ResizeObserver. The app uses one to keep the native video surface aligned with
// the stage element; a no-op stub is enough, since layout never changes under jsdom.
if (!("ResizeObserver" in globalThis)) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}

// Testing Library's auto-cleanup only registers itself when Vitest globals are enabled.
// We keep globals off, so unmount between tests explicitly.
afterEach(cleanup);
