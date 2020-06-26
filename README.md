# Armour

Dynamic authentication and authorisation for "services" using a custom policy language.

### Repository contents

- **`benchmark/`**: Armour performance analysis
- **`design/`**: Armour design documents (markdown and keynote) and project proposal (LaTeX)
- **`examples/`**: examples, including Vagrant development testbed and Vagrant & docker-compose version of the Healthcare PoC
- **`src/`**: Armour source code

### Table of contents

* [Overview](#overview)
* [Documentation](#documentation)
* [Getting started](#getting-started)
* [Micro-benchmark](#micro-benchmark)
* [Future work](#future-work)

<a name="overview"></a>
### Overview

> Armour description.  
> Key features.  
> Design (graph).  

<a name="documentation"></a>
### Documentation
* [Armour paper](https://git.research.arm.com/guspet02/armour-papers.git)
* [Policy language](src/docs/language.md)
<!--- TODO * Architecture --->

<!--- is this all the docs we have? --->

<a name="getting-started"></a>
### Getting started
The `examples/` directory contains a couple of [getting started](examples/README.md) examples. 

<a name="micro-benchmark"></a>
### Micro-benchmark

We tested the performance of the *data plane* against other related solutions (`envoy`, `linkerd` and `nginx`) whilst using various policies and oracles. Here are the [results](benchmark/results/README.md).

<a name="future-work"></a>
### Future work

Armour is still at early stages of development (is it? lol), and trying to solve multiple problems (are we?)
> * Control plane integration.  
> * Proxy injection istio style.    
> * Identity management/root of trust/certificates.  
> * The current version of Armour is best used with unencrypted traffic otherwise there is only so much that can be expressed with the policy language. --> work is begin done to fix this.  
> * Integration with k8s.  
> * Add the stuff inside the direction file.  
