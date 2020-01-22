## Micro-benchmark of Armour data-plan

### Contents

- `containers/`: configuration files of the containers: clients running `wrk2` tool, `hyper` server and configuration files for the proxies used `armour`, `envoy`, `nginx`, `linkerd` and `sozu`.
- `scripts/`: scripts to run the performance analysis.
- `setup/`: scripts to setup the environment (start multiple aws instances and launch the benchmark).
- `results/`: raw data, processed data (has only the info needed) and plots for different benchmark setups and a benchmark of several web servers (`hyper`, `actix-web`, `apache`, `nginx web server`, `cherokee`, `lighttpd`).

### Usage

- Run `./start.sh` in the setup dir.
- After the test are done, run `./get-results.sh` to get the results and produce the graphs.

