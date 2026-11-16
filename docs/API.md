# Core API responsibility

The supported consumer-facing integration contract is the **wire-v1 host protocol**, documented by [`protocol/README.md`](../protocol/README.md). Consumers must not bind directly to private Rust modules.

Key domains: application lifecycle; window lifecycle; document load and command; document event subscriptions; native dialogs; clipboard and external-opening services; accessibility; diagnostics; bounded bitmap resource lifecycle. Request formats, limits, error codes, and feature negotiation belong to the protocol documentation, not this overview.

Changes that require new wire operations or semantics must be proposed as an approved Core feature and accompanied by consumer conformance tests.
