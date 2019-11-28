

# Performance analysis

Micro-benchmark of Armour data-plan

### Contents

- `containers/`: configuration files of the containers: clients running `wrk2` tool, `nginx` servers and configuration files for the proxies used `armour`, `envoy`, `nginx` and `sozu`.
- `scripts/`: scripts to run the analysis
- `setup/`: scripts to setup the environment (start multiple aws instances and start the benchmark),

### Environment

- Aws t2.micro, Amazon Linux 2 AMI (Linux kernel 4.14), 1GB memory, 1 vCPU, 50GB storage.
- nginx version: 1.16.1
- envoy version: 1.12.1
- sozu version: 0.11.0
- linkerd version: 1.7.0
- amrour version: (20Nov)

### Usage

- Run `./start.sh` in the setup dir.
- After the test are done, run `./get-results.sh` to get the results and produce the graphs.
