# Vajra

A distributed, fault-tolerant, in-memory vector database built from first principles.

## Features

- **HNSW Index**: Hierarchical Navigable Small World graph for fast approximate nearest neighbor search
- **Raft Consensus**: Distributed consensus with Pre-Vote extension for partition tolerance
- **Crash Consistency**: Write-Ahead Log with CRC32 checksums for durability
- **Observability**: Structured logging, distributed tracing (OpenTelemetry), Prometheus metrics
- **Single Binary**: Statically linked for easy deployment

## Quick Start

```bash
# Build
cargo build --release

# Run with default configuration
./target/release/vajra

# Run with custom config
./target/release/vajra --config vajra.toml
```

## Project Structure

```
vajra/
├── crates/
│   ├── vajra-common/      # Shared types, errors, config, telemetry
│   ├── vajra-engine/      # HNSW vector index (libvajra)
│   ├── vajra-wal/         # Write-Ahead Log
│   ├── vajra-raft/        # Raft consensus protocol
│   ├── vajra-transport/   # gRPC server and client
│   └── vajra-server/      # Binary entry point
├── proto/                 # Protocol buffer definitions
├── docs/                  # Documentation
├── tests/                 # Integration and simulation tests
└── benches/               # Performance benchmarks
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md) - System design and components
- [Runbook](docs/RUNBOOK.md) - Operational guide

## Development

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Run lints
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt
```

## License

MIT OR Apache-2.0
