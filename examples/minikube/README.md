Getting started with Minikube
=============================

The following example shows how to run Armour on [Minikube](https://minikube.sigs.k8s.io/docs/)

## Minikube setup
Follow the installation steps for your machine
[Installation](https://kubernetes.io/docs/tasks/tools/install-minikube)

Start minikube VM:
`minikube start`

make sure kubectl works on your host:

```sh
host$ kubectl get nodes

NAME       STATUS   ROLES    AGE     VERSION
minikube   Ready    master   6m30s   v1.18.3
```


we need to get both `armour-init` and `armour-proxy` docker images and `armour-control` and `armour-ctl` binaries inside the VM:

```sh
host$ ./setup.sh
host$ minikube ssh 
minikube$ cd armour-init && docker build -t armour-init .
minikube$ cd ../armour-proxy && docker build -t armour-proxy .
minikube$ docker run -d armour-proxy
minikube$ docker cp <containerId>:/home/armour/src/target/release/armour-control .
minikube$ docker cp <containerId>:/home/armour/src/target/release/armour-ctl .
```
## Demo

Open 3 terminal windows, the terminals correspond with the following:
	
   1. **Armour control plane**
   1. **Armour-ctl commands**
   1. **K3s demo application**


**Armour control plane [1]**

I'm using my host's monogodb:

```sh 
host$ sudo mongod --dbpath=/Users/$(whoami)/data/db
host$ minikube ssh
minikube$ cd armour-proxy/
minikube$ ./armour-control -m mongodb://10.0.2.2:27017
```
 
**Armour-ctl commands [2]**

 ```sh
 host$ minikube ssh
 minikube$ cd armour-proxy/
 minikube$ ./armour-ctl update -p allow.policy -s client
 minikube$ ./armour-ctl update -p allow.policy -s server
 ```
 > In case you want to run the egress-ingress demo, run these `armour-ctl` commands instead:  
 **Armour-ctl commands [2]**
 >
 ```sh
 host$ minikube ssh
 minikube$ cd armour-proxy/
 minikube$ ./armour-ctl update -p allow.policy -s client-in
 minikube$ ./armour-ctl update -p allow.policy -s client-eg
 minikube$ ./armour-ctl update -p allow.policy -s server-in
 minikube$ ./armour-ctl update -p allow.policy -s server-eg
 ```

**K3s demo application [3]**

 ```sh
 host$ kubectl apply -f demo.yaml
 ```
  > In case you want to run the egress-ingress demo, run these commands instead:  
 **K3s demo application [3]**
 >
  ```sh
 host$ kubectl apply -f demo-egress.yaml
  ```
 
 At this point, on terminal **[1]** you should see the armour-hosts and proxies getting onboarded. Wait couple of seconds and Run:
 
 **K3s demo application [3]**

 ```sh
 host$ kubectl get all -n armour
 
NAME         READY   STATUS    RESTARTS   AGE
pod/client   2/2     Running   0          6s
pod/server   2/2     Running   0          6s

NAME                     TYPE       CLUSTER-IP     EXTERNAL-IP     PORT(S)                        AGE
service/client-service   NodePort   10.43.194.83   172.42.42.102   80:31124/TCP,32001:32001/TCP   6s
service/server-service   NodePort   10.43.69.237   172.42.42.102   80:31237/TCP,32000:32000/TCP   6s
 ```
 Use the `Cluster-IP` to reach the client and server
 
 ```sh
 host$ kubectl exec -n armour client -c client curl 10.43.69.237
Response!
 ```
 
 **Armour-ctl commands [2]**

 ```sh
 minikube$ ./armour-ctl update -p deny.policy -s client
 ```
  > In case you want to run the egress-ingress demo, run these `armour-ctl` commands instead:  
 **Armour-ctl commands [2]**
 >
 ```sh
 minikube$ ./armour-ctl update -p allow.policy -s client-eg
 ```
 
  **K3s demo application [3]**

 ```sh
 host$ kubectl exec -n armour client -c client curl 10.43.69.237
Request denied!
 ```
 
 Bring the services down:
 **K3s demo application [3]**

 ```sh
 host$ kubectl delete -f demo.yaml
 host$ minikube delete
 ```