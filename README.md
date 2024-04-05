## Description

Simple implementation of p2p network, where every node is connected to every node. Works with 64-bit memory systems and IPv4.

Rust version - 1.77.1.

### Rust project consists of two packages
- node - lib, which provides node and message sending realization;
- src - binary, which runs node, using cli arguments.

### CLI arguments
- period - how often node send messages in seconds, default 1;
- port - what port binding to listen;
- connect (c) - address connect to;
- level (l) - [tracing level](https://docs.rs/tracing/latest/tracing/struct.Level.html#implementations), tracing display to console, default DEBUG.

### How to run
From the root of the project execute to run the binary,
the command will up node, which would be running in solo:

``cargo run -- --period=5 --port=8080 --level=info``


From the root of the project execute to run the binary, the command will up node, which would try to connect to the node at the socket "127.0.0.1:8080":

``cargo run -- --period=10 --port=8081 --connect=127.0.0.1:8080 --level=info``