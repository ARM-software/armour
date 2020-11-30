### Code future work

* Types labels (CP labels and DP labels) or at get ride of one of the to/from (the one dynamically none) and only allow has_label on the CPID
  Howto: TODO
* One policy per label and not per proxy
  Therefore get ride of the limitation of one service per proxy
  Howto: TODO
  Todos:
    - add Proxy::proxy_label to the labels of a service
    - update newID in armour-control/src/interpreter.rs: ServiceID::host_label::proxy_label::service_label 
    - backpropagation policy per µservice TODO
* Add two different types to distinguish between CP and DP labels inside onboaridng/global pol TODO    
* Type for data plane main function 
* Design a multi-layers specializer
* Credentials is the empty string in armour-host/src/host.rs, should be initialize from cmd or rest request
    - change the dummy implementation of verify_credentials in the armour-control/src/interpreter.rs + update return type to have a token
* Add label/rm label on host ? label to proxy or to service (if so propagation) TODO    
* Autoboxing for label TODO
* 
  

### Possible Future Work

> * Proxy injection, istio style
> * Identity management
>     * root of trust
>     * certificates
> * Support for encrypted traffic (see *pangolin*). Currently TLS will mean there is no visibility of requests and responses.
> * Integration with k8s.  
