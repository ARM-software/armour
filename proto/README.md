arm-proxy
---------

The proxy can be started with:

   ```shell
   $ cd arm-proxy
   $ cargo run
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   ```

This service will accept REST requests sent to `http://?.arm.com:8443/{port}/...` and, if `{port}` is not blocked, it will forward the request on to `http://?.arm.com:{port}/...`.

The proxy's port can be changed on the command line as follows:

   ```shell
   $ cargo run 8444
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8444
   ```

All ports are blocked at startup, unless they are unblocked on the command line:

   ```shell
   $ cargo run -- -a 80 -a 8445 -a 8446
   [... INFO  arm_proxy] Allowed ports are: [80, 8445, 8446]
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   ```

The proxy can also start up a pub/sub service using the `-p` flag:

   ```shell
   $ cargo run -- -p 8444
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   [... INFO  arm_proxy] Starting pub/sub broker: http://?.arm.com:8444
   ```

arm-service
-----------

The arm-service can listen on a port by setting the `-o` flag:

   ```shell
   $ cd arm-service
   $ cargo run -- -o 8445
   [... INFO  arm_service] Starting service: http://?.arm.com:8445
   ```

REST PUT requests can be sent to a destination port using the `-d` flag and the `-r` flag can be used to specify a resource path.

Proxy Example
-------------

- Start the proxy

   ```shell
   # terminal 1
   $ cd arm-proxy
   $ cargo run
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   ```

- Start a service on port 8445

   ```shell
   # terminal 2
   $ cd arm-service
   $ cargo run -- -o 8445
   [... INFO  arm_service] Starting service: http://?.arm.com:8445
   ```

- Try to send a message to the service on port 8445

   ```shell
   # terminal 3
   $ cd arm-service
   $ cargo run -- -d 8445 "hello"
   [.. INFO  arm_service] sending: http://?.arm.com:8443/8445/
   403: access to server 8445 is blocked
   ```

- Send a request to proxy to open port 8445

   ```shell
   # terminal 3
   $ cargo run -- -d 8443 -r allow/8445
   [... INFO  arm_service] sending: http://?.arm.com:8443/allow/8445
   200: Added port 8445
   ```
   
   At this point the proxy should also log the fact that access to port 8445 is now allowed.
   
- Try to send a message again

   ```shell
   # terminal 3
   $ cd arm-service
   $ cargo run -- -d 8445 "hello"
   [2019-03-08T11:04:29Z INFO  arm_service] sending: http://?.arm.com:8443/8445/
   200: port 8445 received request / with body b"hello" from ?.arm.com:8443
   ```
   
   This shows that the service on port 8445 replied with an acknowledgement that it received the request.

Pub/Sub Example
---------------

- Start the proxy with pub/sub enabled

   ```shell
   # terminal 1
   $ cd arm-proxy
   $ cargo run -- -p 8444
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   [... INFO  arm_proxy] Starting pub/sub broker: http://?.arm.com:8444
   ```

- Start two services, which subscribe to a topic by messaging the broker on port 8444

   ```shell
   # terminal 2
   $ cd arm-service
   $ cargo run -- -o 8445 -d 8444 -r subscribe/8445/messages
   [... INFO  arm_service] Starting service: http://?.arm.com:8445
   [... INFO  arm_service] sending: http://?.arm.com:8444/subscribe/8445/messages
   200: added subscriber 8445 to topic "messages"
   ```
   
   ```shell
   # terminal 3
   $ cd arm-service
   $ cargo run -- -o 8446 -d 8444 -r subscribe/8446/messages
   [... INFO  arm_service] Starting service: http://?.arm.com:8446
   [... INFO  arm_service] sending: http://?.arm.com:8444/subscribe/8446/messages
   200: added subscriber 8446 to topic "messages"
   ```

- Publish a message to the topic

   ```shell
   # terminal 4
   $ cd arm-service
   $ cargo run -- -d 8444 -r publish/messages "hello there"
   [... INFO  arm_service] sending: http://?.arm.com:8444/publish/messages
   200: published to topic "messages"
   ```
   
   The services 8445 and 8446 should both log receipt of the message.