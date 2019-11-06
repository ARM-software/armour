Armour Policies
===============

## Policies

The `armour-data-master` can set the current policy with the following commands:

- `allow all`: all connections are permitted
- `deny all`: no connections are permitted
- `policy <policy-file>`: a connection is conditional, based on functions defined in `policy-file` (see below for details)

## TCP policies

A connection is only permitted if `policy-file` declares a function

```
fn allow_connection(from: ID, to: ID) -> bool
```
    
and that function returns `true` for the connection endpoints `from` and `to`. The type `ID` is used to encapsulate the hostnames, IP addresses and port number of an endpoint. For a list of `ID` type methods see `language.md`.

### Example

A minimal `allow all` TCP policy is

```
fn allow_connection(from: ID, to: ID) -> bool { true }
```
    
## REST policies

A connection is only permitted `policy-file` declares at least one of the following functions

```
   fn require() -> bool
or fn require(req: HttpRequest) -> bool
or fn require(req: HttpRequest, from: ID, to: ID) -> bool

   fn client_payload(payload: data) -> bool
or fn client_payload(payload: data, from: ID, to: ID) -> bool

   fn server_payload(payload: data) -> bool
or fn server_payload(payload: data, from: ID, to: ID) -> bool
```

The connection will only succeed if *every* declared function returns `true` for that connection. The `to` endpoint will be contacted only when `require` and `client_payload` are either absent or return `true`.

### Example

A minimal `allow all` REST policy is

```
fn require() -> bool { true }
```