# Management Interface prototype decision record

Status: retained primary-source record; not production code.

The throwaway exploration that preceded the React interface was used to decide information architecture and interaction semantics. Its durable findings are recorded here outside `frontend/src` and are enforced by production tests. This file is evidence of the prototype decisions, not a second application or supported UI.

| Prototype question | Decision carried into production | Production evidence |
| --- | --- | --- |
| Can one layout serve small and large screens? | Desktop uses persistent side navigation and inspector; narrow screens use bottom navigation and sheet-like details. | `frontend/src/style.css`, responsive UI tests |
| Where should destructive actions live? | Permanent deletion is separate from ordinary tasks, requires a fresh server plan, explicit path scope, typed phrase, and audit history. | deletion APIs and confirmation dialog tests |
| Should Actor Folder removal look like media deletion? | No. It is labeled derived-path removal and explains that source assets, NFO, and Jellyfin items remain. | Actor Folder dialog/API tests |
| How should long operations survive navigation? | Tasks are durable SQLite records; UI combines snapshots with SSE and can recover after reconnect/restart. | task persistence, interruption, and SSE tests |
| How should metadata exceptions appear? | Asset cards expose state; details separate overview and NFO, show parse errors and actor information. | asset index/detail and responsive frontend tests |
| Where should Rules and Jellyfin live? | Both are authenticated settings. Remote Rules are proposals until validated and activated; Jellyfin credentials remain server-side. | Rules and Jellyfin integration tests |

Rejected prototype ideas were direct browser filesystem access, client-held Jellyfin keys, delete buttons without preview, and a separate web operation ordering. They conflicted with the shared-services, server-authority, and preview/apply model.

The original disposable rendering was intentionally not promoted into the runtime. This record is the canonical retained primary source; future visual experiments belong in this directory and must be clearly marked non-production.
