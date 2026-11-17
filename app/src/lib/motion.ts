// Motion tokens for Svelte transitions. These mirror the `--dur-*` custom
// properties in app.css; Svelte transitions run through the Web Animations API,
// so the CSS reduced-motion override does not reach them. `duration()` applies
// the same rule in script: zero under `prefers-reduced-motion: reduce`.

import { prefersReducedMotion } from "svelte/motion";

export { cubicOut as easeOut, cubicInOut as easeInOut } from "svelte/easing";

export const DURATION = {
  instant: 80,
  fast: 140,
  base: 200,
  slow: 320,
} as const;

export function duration(token: keyof typeof DURATION): number {
  return prefersReducedMotion.current ? 0 : DURATION[token];
}

export function reducedMotion(): boolean {
  return prefersReducedMotion.current;
}
