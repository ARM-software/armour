### Armour: possible directions

In general, we are looking to add control plane features. Possible areas include:

- Identity management
    - Certificates (Istio features)
    - Use of mTLS
    - Zero-trust ideas, e.g. trust model with ML features
    - Links with: Veracruz, remote attestation and Guilhem's project
- Policy management
    - Composition (many to one)
    - Dissemination/distribution (one to many)
    - Versioning & consistency
    - Derivation: logger and/or GUI editor/frontend
    - Oracle management
- Demonstrator(s) and "policy services"
    - Own PoC, based on start home/city scenarios
    - Session tracking
    - Information flow
    - Communication with docker engine
    - Logging and debugging (perhaps some support for SQL-like queries, record-and-replay, etc.)
- Infrastructure
    - Permit master <-> "policy service" communication
    - Better (more dynamic/integrated) iptables solution (on-boarding)
    - k8s, Docker Swarm and Argus
    - Links with Icecap
- Benchmarking
- Beyond μservices
    - IoT / Edge / Devices management?