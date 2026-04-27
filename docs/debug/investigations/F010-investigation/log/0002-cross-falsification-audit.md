---
id: 0002
type: decide
timestamp: 2026-04-26T11:00:00Z
hypothesis_ids: [H1, P8, P3, B2, B3, P2, B1, B8, P4, B5, H11, P5]
supersedes: null
artifacts: []
---

# Cross-falsification audit

For each cluster, every pair of falsification criteria was checked. No pair shares an artifact such that observing one criterion incidentally satisfies another. Specifically:

- `same-observable-with-H1`: H1 = ops-vec inspection; P8 = rect-dimension log. Distinct.
- `dispatch-cluster`: P3 = trait-method invocation; B2 = VFS state; B3 = upstream dispatch. Distinct sites.
- `invalidation-cluster`: P2 = post-transition recompute log; B1 = init-time theme value; B8 = compositor dirty-flag. Distinct.
- `order-config-cluster`: P4 = pinned-ordering symptom check; B5 = pre-populated cache; H11 = assertions-on release; P5 = cfg matrix. Each intervention is independent.

Cross-falsification rule (spec Section 3, Section 5) is satisfied at this point. Any future revision to a falsification_criterion must rerun this audit.
