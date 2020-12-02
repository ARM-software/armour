###  Control Plane possible future work

* Distinguish labels by using to different types (CP labels when added to the global ID and DP labels for proposed labels) 
```rust
    let id = id.add_label(Label::login_time(System::getCurrentTime())); //CP label
    let id = fold x in obd.proposed_labels() {
		acc.add_label(x) //x is a DP label 
	} where acc=id;
```
then only allow has_label, in the global policy, for CP typed labels.
* Update the control flow: create one policy per service and not per proxy, in order to get ride of the limitation of one service per proxy
	Warning: For now, *service* denotes proxy in the control plane source code since the host hydrates the *service* fields with proxy information
	Howto (disjunction of ideas): 
	1. Create a proxy policy that patternmatch according to the service name (that have been added to labels of the ID) => small changes but conditionnal evaluation (DP interpreter) for each request
	2. Update the proxy infrastructure  
    Todos:
    - add Proxy::proxy_label to the labels of a service
    - update newID in armour-control/src/interpreter.rs: ``ServiceID::host_label::proxy_label::service_label``
    - backpropagation policy per µservice 
* Develops the authentification strategy
	Status:
		- credentials fields (string) in OnboardingRequest, Host -- request --> CP
		- credentials is set to the empty string in armour-host/src/host.rs
		- ``ControlPlane::verify_credentials`` -> always accept
  	Todos:
	  	- credentials should be initialized from *cmd* or from a *rest request* (from proxy to Host) or from *armour-compose file* (i.e. rest request mechanisme behind)
        - change the dummy implementation of ``ControlPlane::verify_credentials`` in the armour-control/src/interpreter.rs,
            - add arguments to define an authorisation strategy and initialize it
            - change return type if a token should be added to the global ID (as a specific labels ``AuthToken::**``)
* Do some autoboxing for labels
	PB statement:
	- step0: an intuitive policy to constraint the topology is the following 
	```rust
	fn allow_rest_request(from: ID, to: ID, req: HttpRequest, payload: data) -> bool {
		to.has_label("server") && from.has_label("client")
	}
	```
	however the former policy will always be evaluated to ``false``
	- step1: partial evaluation (CP) for the client µservice
		```rust
	fn allow_rest_request(req: HttpRequest, payload: data) -> bool {
		req.to().has_label("server")
	}
	```
	- step2: when a request arrive to µservice *client*, the policy of step1 will executed
	however the labels assigned during onboarding are only known by the control plane,
	hence the policy is always evaluated to ``false``
	
	Howtos (ideas and drawbacks):
	* Issuing a request to CP to check the existance of the labels => 
		issues: latency + scalability
	* Propagating the whole sets of labels to each host => 
		issues: scalability issue (if host has a limited memory/network bandwith compare to the ID lists), consistency issues (the list will evolved when service join and leave)
	* Having a labels cache at the host level
		issues: consistency 
		tips: can be prepopulated by doing a static analysis of the has_label calls		
	Solving consistency issue: same approach needed than the one to tackle the policy propagation consistency issue 		
* Use dynamic labels to enable provenance tracking and provide a language primitive to this
	```req.track

### Possible Future Work

> * Proxy injection, istio style
> * Identity management
>     * root of trust
>     * certificates
> * Support for encrypted traffic (see *pangolin*). Currently TLS will mean there is no visibility of requests and responses.
> * Integration with k8s.  
