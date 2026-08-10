# Deepnest‑Next — Open‑Core & Commercial Model

> **Status: DRAFT for review.** This proposes where the open/Pro boundary sits and how
> revenue works. It is a decision for the copyright holder(s); nothing here is final until
> approved. Comments welcome on the PR.

## 1. Principle: never cripple the core

The open build must nest **correctly and well on its own.** A hobbled open version loses the
community, the adoption, and the AGPL leverage that make the commercial side possible.
Therefore **Pro = things businesses pay for** (integration, scale, hardware, hosting,
support) — **not** basic correctness or "good enough" results.

## 2. Licensing model

- **Core is `AGPL‑3.0‑or‑later`.** This is both a community licence and the primary
  commercial lever: anyone who cannot accept AGPL — embedding Deepnest‑Next in a proprietary
  product, or running a closed network service on top of it — needs a **separately‑granted
  commercial licence.**
- **Open‑core works via dual‑licensing**, available to us as the copyright holder: the public
  AGPL core plus a commercial grant for proprietary use/extensions. This requires a **CLA**
  from contributors so their contributions are assignable/relicensable.
- The primary revenue lever needs **no feature difference at all** — it is the licence.
  Feature‑gated Pro add‑ons are a *second* lever layered on top.

*(Engine licensing/vendoring detail lives in `rust-nesting-lib/ARCHITECTURE.md §7`.)*

## 3. What is always open (AGPL) — `tier:core`

Everything needed to produce good production nests:

- NFP exact nesting engine, genetic optimisation, compaction, the NFP cache, `rayon`
  parallelism.
- Holes / inner‑fit placement, **optional‑parts knapsack** (fill waste + the rest of the
  working area by value), multi‑sheet, remnants/offcuts, roll & fixed‑working‑area support,
  cut ordering (holes‑before‑outer).
- Core import/export: **SVG, DXF**.
- The desktop app (Tauri) and the **local** HTTP API (`/v1/nest`).

If it decides *whether* or *how well* parts nest, it belongs here.

## 4. What is Pro / commercial — `tier:pro`

Ordered by strategic weight:

1. **The commercial licence itself** — for anyone who can't/won't comply with AGPL. No feature
   difference required; this is the main revenue.
2. **Camera / vision material capture** — capturing irregular material outlines and
   defect/no‑nest zones from a camera. Hardware‑ and vision‑heavy; business‑oriented.
   *(Already marked `PRO` in the UI concept.)*
3. **ERP / automation connectors** — the Odoo (and generic ERP) *order → DXF → nest →
   cut‑ready* pipeline, job queues, and unattended batch automation.
4. **Hosted "upload & pay to nest" SaaS** — the AGPL core *enables* us to run this; the
   hosting/convenience is the paid part.
5. **GPU raster accelerator** *(proposed)* — a "turbo" backend for dense, high‑volume jobs
   (e.g. thousands of battery tabs), dropped in behind the same `NestingBackend` trait.
   **Kept Pro as a performance tier — the open NFP engine stays genuinely good, so this reads
   as "faster," not "crippleware."** *(Open question for review — see §7.)*
6. **Premium formats & services** — some teased formats (AAMA/ASTM textile DXF, proprietary
   CAD) may be Pro; **SVG/DXF stay open.** Plus priority support / SLAs and a curated
   preset/profile library.

## 5. Sponsors ≠ commercial customers

Two different audiences, two different offers:

| | Sponsors (Patreon / donations) | Commercial customers |
|---|---|---|
| **Get** | Early access, feature‑priority votes, a preset/profile library, recognition/badges | Commercial licence, Pro features (§4), support/SLA |
| **Not** | Hard feature‑locks (avoid gating basics behind donations) | — |

Reward sponsors with **access and influence**, not by locking essential functionality.

## 6. How the boundary is enforced

- **Backend/trait seam:** alternative engines (GPU raster) live behind `NestingBackend`, so
  gating is a build/packaging decision, not a fork.
- **Separate crates / plugins** for Pro code keep the AGPL core self‑sufficient and the
  boundary auditable.
- **Issue labels:** `tier:core` (green) vs `tier:pro` (purple) so every roadmap item is
  explicitly placed. Anything unlabeled defaults to `tier:core` until decided.

## 7. Open questions for review (Dexus)

1. **GPU raster: Pro or open?** Proposed Pro (perf tier). Counter‑view: shipping it open could
   accelerate adoption and make Deepnest‑Next the obvious choice. Decide deliberately.
2. **Which teased file formats are Pro vs open** (Lightburn, EPS, HPGL, AI, PDF, textile DXF)?
3. **Where does the line sit on ERP connectors** — is a *reference* Odoo client open, with only
   the productionised/multi‑ERP connectors Pro?
