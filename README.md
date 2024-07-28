# Hydra

Hydra is a project to to build a federated/distributed hypergraph database.

It intends to provide a simple API for building applications atop of it
which can communicate with each other in near real-time to form
a interpersonal shared (hyper)graph with principles
of data-sovereignty, anti-siloization, and realtime collaboration at it's core.

It is very early in this project.

Intended features:

- Recording and querying time-series data for a given topic
- Representing and querying event-sourced state for a given topic
- Representing (hyper)edges which define relatinoships between topics (and embeddings)
  - Hybrid hypergraph/vector paradigm
- Declarative + recursive live-querying
- Multi-homed users which are able to roam between servers or self-host without
  loosing their data or their thought partners

# Getting started

Note that this is very early in development, and it doesn't yet do much of the above, but if you want to see what exists so far you can run the following:

1. Install rust:
   https://rustup.rs/

2. Install cargo watch (useful for dev workflow)

   ```
   cargo install cargo-watch
   ```

3. Start the server:

   ```
   # from the root directory of the repo
   cargo watch -x 'run --bin hydra-server'
   ```

4. Install wasm-pack
   https://rustwasm.github.io/wasm-pack/installer/

5. Compile the web client:

   ```
   cd web
   cargo watch -s 'wasm-pack build --target web --debug'
   ```

6. install `bun` (npm/node might work, but I haven't tested it, and bun is way faster)

   https://bun.sh/docs/installation

   ```
   cd examples/webapp
   bun dev
   ```
