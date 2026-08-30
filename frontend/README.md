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

## Component provenance

The motion primitives under `src/components/motion` and their helpers under
`src/lib` were promoted from rust-jav's repository-owned Management Interface
prototype. They are maintained as source-installed project components under
the repository's MIT license; they are not a separately versioned runtime
dependency. Third-party runtime behavior comes from Motion (MIT), React (MIT),
Lucide (ISC), Tailwind CSS (MIT), clsx (MIT), and tailwind-merge (MIT), with
exact resolved versions recorded in `package-lock.json`.
