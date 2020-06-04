Control Plane
=============

The following demonstrates Armour running with a control plane.

---

### Policies

Three policies, located in `policies/`, will be used in this example.

#### `id.policy`

```
fn allow_rest_request(req: HttpRequest) -> bool {
  let (from, to) = req.from_to();
  server_ok(to) && from.has_label('allowed')
}

fn server_ok(id: ID) -> bool {
  "server" in id.hosts() &&
  if let Some(port) = id.port() {
    port == 80
  } else {
    // default is port 80
    true
  }
}
```

This will allow requests to the host `server` on port `80`, provided the source service is tagged with the label `allowed`.

<center>
![ID](pictures/id.png)
</center>

> Note: only service `client-1` is tagged as `allowed` in the `armour-compose.yml` file.


#### `log.policy`

```
external logger @ "log_sock" {
  fn log(_) -> ()
}

fn allow_rest_request(req: HttpRequest) -> bool {
  logger::log(req);
  true
}
```
All request are accepted and the request details are also sent to a logger service.
<center>
![ID](pictures/log.png)
</center>


#### `method.policy`

```
fn allow_rest_request(req: HttpRequest, payload: data) -> bool {
    let (from, to) = req.from_to();
    server_ok(to) && from.has_label('allowed') &&
    req.method() == "GET" && req.path() == "/private" && payload.len() == 0
}

fn server_ok(id: ID) -> bool {
  "server" in id.hosts() &&
  if let Some(port) = id.port() {
    port == 80
  } else {
    // default is port 80
    true
  }
}

fn allow_rest_response(res: HttpResponse) -> bool {
    res.status() == 200
}
```
This is similar to `id.policy` but it also checks the *method*, *path* and *payload* of the request, as well as the server response.

<center>
![ID](pictures/method.png)
</center>

---

### Setup

1. Setup and start the Vagrant VM, see [README](../README.md).
1. Start four terminal windows and in each `ssh` into the vagrant VM:

   ```shell
   % cd armour/examples
   % vagrant ssh
   ```

	The terminals correspond with the following
	
   1. **Admin**
   1. **Armour control plane**
   1. **Armour data plane**
   1. **Client**


### Example

Perform the following sequence of commands:

1. Start MongoDB and generate `iptables` rules scripts.
	
	**Admin [1]**
	
	```
   $ sudo systemctl start mongod
   $ cd examples/control-plane
   $ armour-launch armour-compose.yml rules
   generated files: rules_up.sh, rules_down.sh, rules_hosts.sh
	```

1. Start the control plane

	**Control plane [2]**

	```
	$ armour-control
	```

1. Install `id.policy` and then query the control plane to check if it is installed.
	
	**Admin [1]**
	
	```
   $ armour-ctl update -p policies/id.policy -s armour
   $ armour-ctl query -s armour
	```

1. Start the data plane

	**Data plane [3]**

	```
	$ ARMOUR_PASS=password armour-master
	```

1. Start the services and apply the `iptables` rules.
	
	**Admin [1]**
	
	```
   $ sudo ./rules_hosts.sh
   $ armour-launch armour-compose.yml up
   $ sudo ./rules_up.sh
	```

1. Make some requests
	
	**Client [4]**
	
	```
   $ docker exec -ti client-1 curl http://server:80
   response!
   $ docker exec -ti client-2 curl http://server:80
   bad client request
	```

1. Change the policy to `log.policy` and start a `logger`
	
	**Admin [1]**
	
	```
   $ armour-ctl update -p policies/log.policy -s armour
   $ logger ../../log_sock
	```

1. Make some requests
	
	**Client [4]**
	
	```
   $ docker exec -ti client-1 curl http://server:80
   response!
   $ docker exec -ti client-2 curl http://server:80
   response!
	```

1. Stop the `logger` and change the policy to `method.policy`
	
	**Admin [1]**
	
	```
   logger:> quit
   $ armour-ctl update -p policies/method.policy -s armour
	```

1. Make some requests

	**Client [4]**
	
	```
   $ docker exec -ti client-1 curl http://server:80
   bad client request
   $ docker exec -ti client-1 curl http://server:80/private
   private area
   $ docker exec -ti client-1 curl --request POST http://server:80/private
   bad client request
   $ docker exec -ti client-1 curl --request GET --data hello http://server:80/private
   bad client request
   $ docker exec -ti client-2 curl http://server:80/private
   bad client request
	```

1. Stop the services and remove the `iptables`rules
	
	**Admin [1]**
	
	```
   $ armour-launch armour-compose.yml down
   $ sudo ./rules_down.sh
	```

1. Stop the data plane

	**Data plane [3]**

	```
	armour-master:> quit
	```

1. Stop the control plane

	**Control plane [2]**

	```
	$ ^C
	```