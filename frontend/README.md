# Management frontend

Install dependencies and start the local frontend against a separately deployed
NAS backend:

```sh
npm ci
VITE_BACKEND_ORIGIN=http://nas.example:8848 npm run dev
```

Alternatively, put `VITE_BACKEND_ORIGIN` in an ignored `.env.local` file. The
Vite development server proxies `/api` and `/health` to that origin. Browser
requests remain same-origin and continue to use the production session cookie
flow. The proxy is not enabled for preview or production builds.

## Embedded production assets

The Rust server embeds the tracked files under `dist/`. Run `npm run build`
after changing any production frontend source, dependency lockfile, or Vite
configuration. The build records a SHA-256 source digest plus the final
`app.js`/`app.css` paths and content hashes in `dist/index.html` and the tracked
`dist/assets/asset-manifest.json`. The Rust build independently verifies the
unique HTML references, manifest, normalized full-shell hash, and bundle bytes.
Normalization empties only the provenance meta values to avoid a self-hash;
any other shell change is rejected. The gate stops with a rebuild command when
source and embedded assets have drifted or a shell/bundle was replaced.

Before handing off a frontend change, run `npm test`, `npm run check`, and
`npm run build`, then compile or test Rust so the embedded-asset gate is also
exercised.

## CSS ownership

`src/design-system.css` is the foundation layer and owns Tailwind integration,
semantic tokens, and reusable `ui-*` controls. `src/style.css` is the single
canonical application-presentation layer for shell layout and feature
components. The Issue #44 CSS audit rejects selector ownership shared between
those layers and duplicate critical selectors within one cascade context.

## Component provenance

The motion primitives under `src/components/motion` and their helpers under
`src/lib` were promoted from rust-jav's repository-owned Management Interface
prototype. They are maintained as source-installed project components under
the repository's MIT license; they are not a separately versioned runtime
dependency. Third-party runtime behavior comes from Motion (MIT), React (MIT),
Lucide (ISC), Tailwind CSS (MIT), clsx (MIT), and tailwind-merge (MIT), with
exact resolved versions recorded in `package-lock.json`.
