# beUI registry component boundary

The Management Interface keeps the embedded React/Vite architecture from ADR 0001.
`frontend/components.json` is the local shadcn registry manifest; its `@beui`
entry documents the source registry and its aliases are the only import boundary
for generic controls.

## Primitives

`@/components/ui` owns Button, Input, Card, Progress, Dialog, Sheet, Dropdown
Menu and Toast. Dialog/Sheet and Toast adapt the already-vendored beUI prototype
components (`MorphingModal` and `AnimatedToastStack`); the remaining primitive
APIs follow shadcn's compositional surface and use the semantic tokens in
`design-system.css`.

Application code may make thin wrappers only when they express an established
domain action or safety rule (for example, an Operation Plan confirmation). A
thin wrapper must delegate keyboard handling, focus treatment, disabled state,
and visual variants to a `ui` primitive.

Do not add a hand-written generic button, input, dialog, menu, toast, card, or
progress control in a feature component. Add or adapt the primitive first,
including accessible behavior and reduced-motion support, then compose it in the
feature. Feature CSS may describe layout and domain presentation but may not
redefine generic-control defaults.

## Verification contract

The login and both desktop/mobile navigations consume `Button`; login and
search fields consume `Input`; global feedback consumes the vendored Toast.
The UI foundation tests cover keyboard/focus semantics and reduce-motion-aware
registry components. Existing responsive CSS remains the source of truth for
390px, 768px, and 1440px layouts.
