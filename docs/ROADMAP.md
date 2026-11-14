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
- [ ] Add origin-aware local-file and network resource permissions.
- [ ] Validate redirect destinations and block private-network access for remote documents.
- [ ] Add bounded concurrency, dependency-count, and compile-time budgets.
- [ ] Run cargo fmt, cargo check, and cargo test in CI.

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
