arm-proxy
---------

The proxy can be started with:

   ```shell
   $ cd arm-proxy
   $ cargo run
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   [... INFO  arm_proxy] Starting proxy controller: http://?.arm.com:8444
   ```

The proxy will listen on port 8443 and will forward on any request that has a valid `forward-to-port` header. However, this forwarding will be blocked by default. To enable access you can either use the `-a` flag or send a `/allow/{port}` request to the controller. The proxy port can be set with the `-p` flag and the controller port can be set with `-o`.


arm-pubsub
----------

The REST based pub/sub service can be started with:

   ```shell
   $ cd arm-pubsub
   $ cargo run
   [... INFO  arm_pubsub] Starting pub/sub broker: http://?.arm.com:8444
   ```

arm-service
-----------

The arm-service can listen on a port by setting the `-o` flag:

   ```shell
   $ cd arm-service
   $ cargo run -- -o 8445
   [... INFO  arm_service] Starting service: http://?.arm.com:8445
   ```

REST GET requests can be sent to a destination port using the `-d` flag and the `-r` flag can be used to specify a resource path. A `forward-to-port` header can be added using the `-f` flag.

Proxy Example
-------------

- Start the proxy

   ```shell
   # terminal 1
   $ cd arm-proxy
   $ cargo run
   [... INFO  arm_proxy] Allowed ports are: []
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   [... INFO  arm_proxy] Starting proxy controller: http://?.arm.com:8444
   ```

- Start a service on port 8445

   ```shell
   # terminal 2
   $ cd arm-service
   $ cargo run -- -o 8445
   [... INFO  arm_service] Starting service: http://?.arm.com:8445
   ```

- Try to forward a message to the service via the proxy

   ```shell
   # terminal 3
   $ cd arm-service
   $ cargo run -- -d 8443 -f 8445 hello
   [.. INFO  arm_service] sending: http://?.arm.com:8443/8445/
   403: access to server 8445 is blocked
   ```

- Send a request to proxy to open port 8444

   ```shell
   # terminal 3
   $ cargo run -- -d 8444 -r allow/8445
   [... INFO  arm_service] sending: http://?.arm.com:8444/allow/8445
   200: Added port 8445
   ```
   
   At this point the proxy should also log the fact that access to port 8445 is now allowed.
   
- Try to send a message again

   ```shell
   # terminal 3
   $ cd arm-service
   $ cargo run -- -d 8443 -f 8445 hello
   [... INFO  arm_service] sending: http://?.arm.com:8443
   200: port 8445 received request / with body b"hello" from ?.arm.com:8443
   ```
   
   This shows that the service on port 8445 replied with an acknowledgement that it received the request (via the proxy at 8443).

Pub/Sub Example
---------------

- Start the proxy with pub/sub on 8445 allowed

   ```shell
   # terminal 1
   $ cd arm-proxy
   $ cargo run -- -a 8445
   [... INFO  arm_proxy] Allowed ports are: [8445]
   [... INFO  arm_proxy] Starting proxy server: http://?.arm.com:8443
   [... INFO  arm_proxy] Starting proxy controller: http://?.arm.com:8444
   ```

- Start the pub/sub service

   ```shell
   # terminal 2
   $ cd arm-pubsub
   $ cargo run
   [... INFO  arm_pubsub] Starting pub/sub broker: http://?.arm.com:8444
   ```

- Start two services, which subscribe to a topic by messaging the broker on port 8445

   ```shell
   # terminal 3
   $ cd arm-service
   $ rm-service $ cargo run -- -o 8446 -d 8443 -f 8445 -r subscribe/8446/messages
   [... INFO  arm_service] Starting service: http://?.arm.com:8446
   [... INFO  arm_service] sending: http://?.arm.com:8443/subscribe/8446/messages
   200: added subscriber 8446 to topic "messages"
   ```
   
   ```shell
   # terminal 4
   $ cd arm-service
   $ cargo run -- -o 8447 -d 8443 -f 8445 -r subscribe/8447/messages
   [... INFO  arm_service] Starting service: http://?.arm.com:8447
   [... INFO  arm_service] sending: http://?.arm.com:8443/subscribe/8447/messages
   200: added subscriber 8447 to topic "messages"
   ```

- Publish a message to the topic

   ```shell
   # terminal 5
   $ cd arm-service
   $ cargo run -- -d 8443 -f 8445 -r publish/messages "hello there"
   [... INFO  arm_service] sending: http://?.arm.com:8445/publish/messages
   200: published to topic "messages"
   ```
   
   The services 8446 and 8447 should both log receipt of the message.