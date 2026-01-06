# Vajra ⚡

A distributed, fault-tolerant, in-memory vector database built from first principles in Rust.

[![Tests](https://img.shields.io/badge/tests-110%20passing-brightgreen)](https://github.com/DevWizard-Vandan/Vajra)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)

---

## Architecture

```
                                    ┌─────────────────────────────────────────────────────────┐
                                    │                      VAJRA NODE                         │
                                    │                                                         │
┌──────────┐     ┌──────────────┐   │   ┌─────────────┐    ┌──────────────────────────────┐  │
│          │     │              │   │   │             │    │         STATE MACHINE        │  │
│  Client  │────▶│    gRPC      │───┼──▶│   Reactor   │───▶│  ┌────────┐    ┌─────────┐  │  │
│          │     │   Server     │   │   │  (biased    │    │  │  HNSW  │    │   WAL   │  │  │
└──────────┘     └──────────────┘   │   │   select!)  │    │  │ Index  │    │  (Disk) │  │  │
                                    │   │             │    │  │(Memory)│    │         │  │  │
                                    │   └──────┬──────┘    │  └────────┘    └─────────┘  │  │
                                    │          │           └──────────────────────────────┘  │
                                    │          │                                             │
                                    │          ▼                                             │
                                    │   ┌─────────────┐                                      │
                                    │   │  Transport  │                                      │
                                    │   │  Manager    │                                      │
                                    │   └──────┬──────┘                                      │
                                    │          │                                             │
                                    └──────────┼─────────────────────────────────────────────┘
                                               │
                    ┌──────────────────────────┼──────────────────────────┐
                    │                          │                          │
                    ▼                          ▼                          ▼
             ┌─────────────┐            ┌─────────────┐            ┌─────────────┐
             │   Node 2    │◀──────────▶│   Node 1    │◀──────────▶│   Node 3    │
             │  (Follower) │    Raft    │  (Leader)   │    Raft    │  (Follower) │
             └─────────────┘  Consensus └─────────────┘  Consensus └─────────────┘
```

### Data Flow

1. **Client** sends gRPC request (Search, Upsert, Delete)
2. **Reactor** receives request via channel (never blocks gRPC thread)
3. **Raft** replicates write operations to followers
4. **WAL** persists committed entries to disk (crash recovery)
5. **HNSW** applies changes to in-memory index (fast search)

---

## Why These Choices?

### Why HNSW?

> **O(log N)** approximate nearest neighbor search complexity.

HNSW (Hierarchical Navigable Small World) provides:
- Sub-millisecond search on million-scale datasets
- Tunable accuracy/speed tradeoff via `ef_search` parameter
- No need for expensive re-indexing on updates

### Why Raft?

> **CP guarantees** (Consistency + Partition tolerance) over AP.

For a vector database where writes must be durable:
- Strong consistency prevents "phantom" vectors
- Leader-based replication simplifies conflict resolution
- Pre-Vote extension prevents term inflation during partitions

### Why Rust?

> **Zero GC pauses** during vector search hot paths.

Rust provides:
- No "stop-the-world" garbage collection during search
- Predictable latency for P99 SLAs
- Memory safety without runtime overhead
- Fearless concurrency with the borrow checker

---

## Features

- **HNSW Index**: Fast approximate nearest neighbor search
- **Raft Consensus**: Distributed consensus with Pre-Vote extension
- **Crash Consistency**: Write-Ahead Log with CRC32 checksums
- **Biased Event Loop**: Heartbeats never wait for searches
- **gRPC Transport**: Streaming upsert, reflection support
- **Observability**: OpenTelemetry tracing, Prometheus metrics

---

## Quick Start

```bash
# Build
cargo build --release

# Run single node
./target/release/vajra --node-id 1 --listen 127.0.0.1:50051

# Run 3-node cluster (trinity demo)
.\scripts\trinity_demo.ps1 -Clean
```

---

## Project Structure

```
vajra/
├── crates/
│   ├── vajra-common/      # Shared types, errors, config
│   ├── vajra-engine/      # HNSW vector index
│   ├── vajra-wal/         # Write-Ahead Log
│   ├── vajra-raft/        # Raft consensus protocol
│   ├── vajra-transport/   # gRPC server and client
│   └── vajra-server/      # Binary + Reactor
├── proto/                 # Protocol buffer definitions
├── configs/               # Node configuration files
└── scripts/               # Demo and utility scripts
```

---

## Test Results

```
110 passed; 0 failed

vajra-common:    19 tests
vajra-engine:    29 tests
vajra-wal:       18 tests
vajra-transport: 14 tests
vajra-raft:      12 tests
vajra-server:     7 tests
doc-tests:       11 tests
```

---

## Development

```bash
cargo build --workspace     # Build all
cargo test --workspace      # Run tests
cargo clippy --workspace    # Lint
cargo fmt                   # Format
```

---

## License

MIT OR Apache-2.0
