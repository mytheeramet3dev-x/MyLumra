# Lumra Resource Security Model (Phase 0)

## Status

This document records the initial security boundary and the first resource-exhaustion protections. It is not a claim that Lumra is production-secure.

## Resource limits implemented

The Phase 0 resource fetcher currently enforces:

- A 32 MiB maximum body size for top-level HTTP(S) documents.
- A 32 MiB maximum size for local text documents and fetched assets.
- A 128 MiB aggregate in-memory asset-cache budget.
- Oldest-insertion eviction when the cache budget would be exceeded.
- A cache bypass for individual entries larger than the cache budget.

Both declared HTTP content length and the actual downloaded body size are checked. The actual size check is required because servers may omit or misreport Content-Length.

## Security work still required

The current fetcher is not yet a safe sandbox for untrusted documents. In particular:

1. **Network address classification needs an independent security review.** Remote HTTP(S) requests resolve hostnames before connecting, reject common private, loopback, link-local, multicast, carrier-grade NAT, benchmarking, documentation, IPv6 translation/tunneling, and internal-host destinations, and pin the client to the validated address set to reduce DNS-rebinding risk. Redirects are disabled. The special-use address list should still be audited against current IANA registries and tested on every supported platform.
2. **Timeouts and concurrency need budgets.** Per-request timeouts alone do not bound aggregate concurrent work.
3. **Compiler and renderer execution budgets are partial.** Each browser instance admits at most two background Typst compilation/navigation workers at a time; excess work is rejected immediately rather than queued.  Each Typst document is limited to 256 imported resources and 64 MiB of aggregate imported source/asset data. Rendering is limited to 128 pages, scale up to 4x, 24 megapixel estimated output per page, and 128 megapixels estimated output per document. Explicit dependency-depth, compile-duration, and total compiler memory limits are not yet enforced.
4. **Cache policy needs stronger semantics.** The current insertion-order eviction is bounded FIFO, not LRU; cache keys and invalidation should be reviewed before persistent caching is introduced.
5. **File URL portability needs review.** Local file URLs currently use a simple path conversion; percent-encoding and platform-specific file URL forms need dedicated tests.

## Required security principle

A remote document must never inherit the local user's filesystem or private-network authority merely because the browser process can access those resources.

Local documents may be granted local-file capabilities only through an explicit user action and a narrowly scoped policy. Remote documents should receive network capabilities constrained by origin and resource policy.

## Non-goals

- This change does not add HTML compatibility.
- This change does not claim that Typst itself is a complete web application runtime.
- This change does not enable arbitrary scripts or executable code in documents.
