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

1. **Local file access is not origin-scoped.** A remote Typst document may attempt to resolve local file resources. The next security milestone must distinguish user-opened local documents from remote documents and enforce a per-document resource capability policy.
2. **Redirect targets are not re-authorized.** HTTP redirects need hop-by-hop validation, including DNS/IP checks, before requests are followed.
3. **Private-network destinations need blocking.** Loopback, link-local, private, multicast, and cloud metadata destinations must be denied for untrusted remote documents. DNS resolution and rebinding need explicit treatment.
4. **Timeouts and concurrency need budgets.** Per-request timeouts alone do not bound aggregate concurrent work.
5. **Import recursion and compilation work need limits.** The compiler pipeline needs limits for dependency depth/count, total fetched bytes, compile duration, and memory.
6. **Cache policy needs stronger semantics.** The current insertion-order eviction is bounded FIFO, not LRU; cache keys and invalidation should be reviewed before persistent caching is introduced.

## Required security principle

A remote document must never inherit the local user's filesystem or private-network authority merely because the browser process can access those resources.

Local documents may be granted local-file capabilities only through an explicit user action and a narrowly scoped policy. Remote documents should receive network capabilities constrained by origin and resource policy.

## Non-goals

- This change does not add HTML compatibility.
- This change does not claim that Typst itself is a complete web application runtime.
- This change does not enable arbitrary scripts or executable code in documents.
