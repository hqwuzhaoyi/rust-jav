# beUI registry component boundary

The Management Interface keeps the embedded React/Vite architecture from ADR 0001.
`frontend/components.json` is the local shadcn registry manifest; its `@beui`
entry documents the source registry and its aliases are the only import boundary
for generic controls.

## Primitives

`@/components/ui` owns Button, Input, Textarea, Checkbox, Select, Card,
Progress, Tabs, Dialog, AlertDialog, Sheet, Dropdown Menu, Toggle Group,
Accordion, Scroll Area, Badge, Separator and Toast. Dialog/Sheet, Toast, and
Tabs adapt the already-vendored beUI prototype components (`MorphingModal`,
`AnimatedToastStack`, and motion tabs); the remaining primitive APIs follow
shadcn's compositional surface and use the semantic tokens in
`design-system.css`.

Application code may make thin wrappers only when they express an established
domain action or safety rule (for example, an Operation Plan confirmation). A
thin wrapper must delegate keyboard handling, focus treatment, disabled state,
and visual variants to a `ui` primitive.

Do not add a hand-written generic button, input, textarea, checkbox, select,
dialog, menu, toggle, accordion, toast, card, badge, separator, scroll area, or
progress control in a feature component. Add or adapt the primitive first,
including accessible behavior and reduced-motion support, then compose it in the
feature. Feature CSS may describe layout and domain presentation but may not
redefine generic-control defaults.

## Verification contract

The login and both desktop/mobile navigations consume `Button`; login and
search fields consume `Input`; global feedback consumes the vendored Toast.
Dialog, Sheet, and AlertDialog own their modal role, focus movement, focus trap,
Escape, and opener restoration, so feature code provides domain content rather
than accessibility mechanics. The UI foundation tests cover keyboard/focus
semantics and reduced-motion-aware registry components. Existing responsive CSS
remains the source of truth for 390px, 768px, and 1440px layouts.

## Enforcement

`npm run lint` parses production `.tsx` with the TypeScript compiler API. It
rejects raw `button`, `input`, `textarea`, and `select` elements plus generic
or dynamically computed interaction roles (including Dialog, menu, tab, and
progress roles) outside the approved registry implementation roots:
`src/components/ui` and the vendored beUI motion sources in
`src/components/motion`. Application and feature modules may import generic
interactions only through `components/ui`. Semantic content HTML remains allowed: headings, text,
lists, `label`, links, native `details`/`summary`, regions, status/alert
announcements, images, and form structure are not generic-control
reimplementations.

There is no migration allowlist: every production finding fails lint. The
legacy `ControlButton` and `beui-tabs.tsx` compatibility surfaces have been
removed. `MorphingModal` and `AnimatedToastStack` remain private vendored beUI
implementations behind the registry Dialog/Sheet/AlertDialog and Toast seams;
feature modules must never import them directly.
