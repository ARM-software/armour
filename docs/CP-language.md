## Control flow
0. In the following, we assume that a CP (+ a DB) and at least one host are running
1. Set up the global policy
2. Set up the onboarding policy (by default on_boarding is disabled)
3. Services can now be onbarded
4. Onboard the *µ1* service with armour-launch
    1. Write the armour-compose file. **Warning** until further modification of the proxy code, only one policy per proxy can be set (therefore one service per proxy is a good architecture)
    2. ``armour-launch ... up`` -- service information + proxy information --> host
    3. Host will start the proxy, onboard it localy, trigger the onboarding with the CP
    4. Host -- OnboardingServiceRequest (service information, proxy label, host label) --> CP
    5. The handling of the request will be done in three steps
        1. The control plane will evaluate the onboarding policy on the request
        2. If the onboarding is allowed, the services collections, in DB, will be updated and a specialized version of the policy will be computed for µ1 and stored.
        3. Then the Control Plane sends back a response with µ1 globalID
    6. If the onboarding failed, send back an error. **The onboarding stops**
    7. Otherwise, the Proxy will request the policy for the *µ1* and will update its local state (see the previous Warning).
5. Update the global-policy
    1. Update the DB
    2. If the optionnnal *selector* argument is provided then this change should translated to local policy updates for the µservices designed by the *selector*
        1. Compute the list of the targeted services
        2. For each of them, specialize the global policy and send the new local policy to it
        3. **Warning** there is no mechanism to ensure atomicity (or a weaker level of consistency) of the update.
6. Update the onboarding-policy -> it only changes the onboarding_policy in the DB and affects subsequent onboarding.

### Ctl
    * armour-ctl drop-global
    * armour-ctl drop-onboarding
    * armour-ctl query-onboarding
    * armour-ctl query-global
    * armour-ctl update-global -p policies/global-id.policy
    * armour-ctl update-onboarding -p policies/onboarding.policy

### Global ID assignement
GlobalID of a service is computed by concatenated the Host label, the Proxy label and the Server label.
A GlobalID is unique among all the onboarded services and services that try to on-board.

GlobalID management during onboarding_policy evaluation:
    - obd -- ControlPlane::onboarded --> Option<GlobalID>
    - obd -- ControlPlane::newID --> GlobalID 
    - GlobalID -- ControlPlane::newID --> bool, store GlobalID on DB

GlobalID flow
1. Host -- OnboardingServiceRequest --> CP, generates obd
2. obd -- evaluating onboarding_policy --> GlobalID + local_policy 
3. CP adds to policies collection (in DB): ``GlobalID -> local_policy``
4. CP adds to services collection (in DB): ``GlobalID + service information``
5. CP -- OnboardingServiceResponse (GlobalID) --> Host -- ... --> Proxy 

Management of local policies is done by using the GlobalID and not the service name:
* ``armour-ctl query -s globalid``
* ``armour-ctl drop -s globalid``
* ``armour-ctl update -s globalid``

##### N.B
By using the services and the policies collection, the control plane is aware of what policy is deployed where. However, since consistency is not tackled by the implementation, there can be glitch between what is declared in the DB and what is actually run on the proxy due to the propagation latency.


## CP Language

### Literals and Types

    Function        |   Types           
--------------------|-------------------
Credentials         | Credentials       
OnboardingData      | OnboardingData    
OnboardingResult    | OnboardingResult  
Policy              | Policy            
Primitive           | Primitive         

#### Credentials::

#### Labels::

    Function        |   Types           
--------------------|-------------------
Label::login_time   | `i64 -> Label`

#### OnboardingData::

    Function        |   Types           
--------------------|--------------------------------
proposed_labels     | `OnboardingData -> List<Label>`
has_proposed_label  | `(OnboardingData, Label) -> bool`
has_ip              | `(OnboardingData, IpAddr) -> bool`
host                | `OnboardingData -> Label`
service             | `OnboardingData -> Label`

#### OnboardingResult::

    Function        |   Types           
--------------------|-------------------------------------------
Ok                  | `(ID, Policy, Policy) -> OnboardingResult`
Err                 | `String -> OnboardingResult`
ErrID               | `(String, ID) -> OnboardingResult`
ErrStr              | `(String, ID, Policy, Policy) -> OnboardingResult`

#### ControlPlane primitives
    Function                        |   Types                           
------------------------------------|-----------------------------------
ControlPlane::verify_credentials    | `(OnboardingData, Label) -> bool` 
ControlPlane::onboarded             | `OnboardingData -> Option<ID>`    
ControlPlane::newID                 | `OnboardingData -> ID`            
ControlPlane::onboard               | `ID -> bool`                      

#### Policy primitives
    Function                        |   Types                       
------------------------------------|-------------------------------
allow_egress                        | `() -> Policy`                
allow_ingress                       | `() -> Policy`                
deny_egress                         | `() -> Policy`                
deny_ingress                        | `() -> Policy`                
compile_egress                      | `(Primitive, ID) -> Policy`   
compile_ingress                     | `(Primitive, ID) -> Policy`   
Primitive::allow_rest_request       | `() -> Primitive`             
Primitive::allow_rest_response      | `() -> Primitive`             
Primitive::on_tcp_disconnect        | `() -> Primitive`             
Primitive::allow_tcp_connection     | `() -> Primitive`             

#### System::

    Function        |   Types           
--------------------|-------------
getCurrentTime      | `() -> i64`

### onboarding policy evaluation

The evaluation of the onboarding may involve state (DB) access and update when using the ``ControlPlane::*`` primitives.

### global policy specialization

1. Partial evaluation
2. Inlining (N.B: oracles are not inlined)
3. Constant folding and propagation
4. Simplifications passes:
    * Boolean/arithmetic simplification
    * Constant propagation
    * Dead-code elimination:
    * Conditional/binder elimination

