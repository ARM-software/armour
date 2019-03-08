TODO
====

arm-proxy
---------

The REST proxy service needs to support the following:

- Enable filtering based on: sender `ip` and/or `id`; recipient `ip` and `port`, and/or `id`; `API/route`; and `body`.

The pub/sub service needs to support the following:

- Topics are currently simple strings; these should be replaced by paths.
- Control/block access based on topic path regexs (in preparation for policy control).
- Messages are not being timestamped or stored. The service should be backed up by a proper database (message archive).
- The pub/sub API probably needs to be extended? (What's actually needed for the Healthcare PoC?)
- Robustness (tolerate unreliable connectivity)?

Other features:

- Ensure only trusted "control plane" component can update/manage policies.
- Integration with policy language. Pattern matching on API and topic.
- Move to TLS and identity management. (Tokens?)
- Work with k8s.
- Facility for opening up point-to-point connections.