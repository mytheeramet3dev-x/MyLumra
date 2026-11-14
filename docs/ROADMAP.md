# MyLumra Development Roadmap

## Product decision

- Strategy: evolve the existing MyLumra codebase.
- Web compatibility: no HTML compatibility layer; Lumra is intended to be Typst-native.
- Implementation language: Rust.
- Existing renderer and Typst integration are retained while their boundaries are improved.

## Phase 0 — Stabilization

- [x] Add per-resource size limits to remote documents, local documents, and fetched assets.
- [x] Bound the in-memory asset cache to 128 MiB with FIFO eviction.
- [x] Add unit tests for cache eviction and oversized cache entries.
- [x] Deny local-file access to remote documents.
- [x] Restrict local document resources to the canonical directory of the opened document.
- [x] Disable automatic HTTP redirects pending destination validation.
- [x] Add GitHub Actions CI for cargo fmt, cargo check, and cargo test.
- [x] Resolve and pin remote DNS results; block private and common special-use network ranges.
- [ ] Independently audit the IP classification policy against IANA special-purpose registries.
- [x] Bound concurrent remote page and asset requests with a shared eight-request limit and fail-fast admission.
- [ ] Add per-origin fairness, cancellation, and retry scheduling.
- [x] Limit each Typst document to 256 imported resources and 64 MiB of aggregate imported data.
- [x] Limit rendering to 128 pages, 4x scale, 24 MP estimated per page, and 128 MP estimated per document.
- [ ] Enforce dependency-depth, compile-duration, and total compiler memory budgets.
- [ ] Confirm CI passes on the Phase 0 branch.

## Phase 1 — Lumra Core

- Define a stable document/resource model.
- Separate navigation, fetching, compilation, layout, and presentation.
- Add deterministic URL and dependency resolution.
- Introduce structured diagnostics and observable performance metrics.

## Phase 2 — Lumra Runtime

- Specify component identity and lifecycle.
- Add a typed event model and reactive state updates.
- Add accessible native controls and form semantics.
- Keep document evaluation deterministic and capability-based.

## Phase 3 — Typst-native Web Standard

- Specify the .typ web document profile.
- Define routing, metadata, links, forms, component contracts, and resource declarations.
- Publish a versioned conformance suite and reference examples.
- No HTML compatibility layer is planned.

## Phase 4 — Ecosystem

- Package/module registry with integrity and provenance.
- Developer tooling, diagnostics, preview, and profiling.
- Static hosting and deployment conventions.
- Search and indexing for Lumra-native documents.

## Phase 5 — Internet Platform

- Interoperable hosting and caching.
- Optional content-addressed distribution.
- Federation and decentralized publishing, with explicit availability and trust guarantees.

## Engineering policy

Every milestone must have a written design, tests, measurable acceptance criteria, and a review of security and backward compatibility within the Lumra ecosystem before merge.
