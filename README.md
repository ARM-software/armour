# Armour

Dynamic authentication and authorisation for "services" using a custom policy language.

### Repository contents

- **`benchmark/`**: performance analysis
- **`design/`**: design documents (markdown and keynote) and project proposal (LaTeX)
- **`docs/`**: documentation
- **`examples/`**: examples, including Vagrant development testbed and Vagrant & docker-compose version of the Healthcare PoC
- **`src/`**: source code

### Documentation

The **`docs/`** directory provides some [documentation](docs/README.md).

### Getting started
The **`examples/`** directory contains a few getting started [examples](examples/README.md). 

### Micro-benchmarking

The performance of the *data plane* was tested against other related solutions (`envoy`, `linkerd` and `nginx`) using various policies and oracles.

The **`benchmark/`** directory provides benchmarking scripts and presents the [results](benchmark/results/README.md).
