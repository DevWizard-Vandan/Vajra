# Vajra Architecture

> "If you can't trace it, it doesn't exist."
> "Build it correct, make it fast, make it small — in that order."
> "The heartbeat must never wait for a search."

## Overview

Vajra is a distributed, fault-tolerant, in-memory vector database built from first principles. It implements HNSW (Hierarchical Navigable Small World) for approximate nearest neighbor search, Raft consensus for replication, and a custom Write-Ahead Log for crash consistency.

## Design Philosophy

### Non-Negotiable Constraints

1. **No Black-Box Dependencies**: We build the indexing engine, consensus protocol, and transport layer ourselves. No FAISS, Annoy, ScaNN, Etcd, or Zookeeper.

2. **Single Binary Distribution**: The final artifact is a statically-linked binary via musl.

3. **Deterministic Builds**: All dependency versions are pinned in `Cargo.lock` with the compiler version locked via `rust-toolchain.toml`.

### Architectural Invariants

1. **Decoupling**: The search engine (`libvajra` / `vajra-engine`) compiles and functions as a standalone library with zero network dependencies.

2. **Observability**: Every cross-node request carries a distributed trace ID (W3C Trace Context format).

3. **Linearizability**: All reads can reflect the most recently committed write (via Read Index or Lease Reads).

4. **Crash Consistency**: No acknowledged write is lost, even under power failure mid-write.

## Component Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        vajra-server                              │
│                    (Binary Entry Point)                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐ │
│  │   Raft      │    │   gRPC      │    │     Metrics         │ │
│  │   Tick      │    │   Server    │    │     HTTP            │ │
│  │   Loop      │    │             │    │     Endpoint        │ │
│  └──────┬──────┘    └──────┬──────┘    └─────────────────────┘ │
│         │                  │                                     │
│         ▼                  ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                     vajra-raft                               │ │
│  │              (Consensus Protocol)                            │ │
│  │    Leader Election │ Log Replication │ Pre-Vote │ Snapshots │ │
│  └──────────────────────────┬──────────────────────────────────┘ │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                      vajra-wal                               │ │
│  │              (Write-Ahead Log)                               │ │
│  │      CRC32 Checksums │ Segment Rotation │ Crash Recovery    │ │
│  └──────────────────────────┬──────────────────────────────────┘ │
│                              │ (apply)                           │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    vajra-engine                              │ │
│  │                (HNSW Vector Index)                           │ │
│  │      Insert │ Search │ Delete │ Distance Metrics │ SIMD     │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    vajra-common                              │ │
│  │    Types │ Errors │ Config │ Telemetry │ Observability      │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Responsibilities

### vajra-common
Shared infrastructure used by all other crates:
- **Types**: `VectorId`, `NodeId`, `Metadata`, `SearchResult`
- **Errors**: Comprehensive `VajraError` enum covering all failure modes
- **Config**: Configuration structures for all components
- **Telemetry**: Tracing, OpenTelemetry, and Prometheus metrics

### vajra-engine
Standalone vector indexing library (no network dependencies):
- HNSW graph structure and algorithms
- Multiple distance metrics (Euclidean, Cosine, Inner Product)
- Concurrent read/write model with DashMap
- Soft deletes for Raft determinism
- SIMD-optimized distance calculations (Phase 6)

### vajra-wal
Crash-consistent persistence:
- Binary log entry format with CRC32 checksums
- Segment rotation and compaction
- Configurable sync policies (every entry, batched, OS default)
- Fast recovery with partial write detection

### vajra-raft
Raft consensus implementation:
- Leader election with randomized timeouts
- Log replication with conflict detection
- Pre-Vote extension to prevent term inflation
- Read Index for linearizable reads
- Snapshot management

### vajra-transport
gRPC API and inter-node communication:
- Vector operations: Upsert, Search, Delete, Get
- Streaming for bulk operations
- Raft RPCs: RequestVote, AppendEntries, InstallSnapshot
- W3C Trace Context propagation

### vajra-server
Binary orchestration:
- CLI argument parsing
- Configuration loading and validation
- Startup sequencing (WAL recovery → State rebuild → Raft init → Network)
- Graceful shutdown handling

## Data Flow

### Write Path
```
1. Client → gRPC Upsert → Leader
2. Leader proposes to Raft log
3. Log entry appended to WAL (fsync)
4. Entry replicated to followers
5. Majority ack → Entry committed
6. State machine applies: HNSW insert (deterministic)
7. Response to client with log index
```

### Read Path (Linearizable)
```
1. Client → gRPC Search → Any Node
2. If linearizable requested:
   a. Read Index: Confirm leadership, get commit index
   b. Wait for commit index to be applied
3. Execute HNSW search
4. Return results with read_at_index
```

## Key Design Decisions

### Why Build HNSW From Scratch?
- Full control over determinism for Raft state machine
- Understanding the algorithm enables better debugging
- No dependency version conflicts or licensing issues

### Why Pre-Vote?
- Prevents partitioned nodes from incrementing terms
- Avoids cluster disruption when partition heals
- Standard extension recommended by Raft authors

### Why Soft Deletes?
- Graph modification order must be deterministic
- Actual graph restructuring deferred to background compaction
- Simpler Raft state machine

### Why Single Binary?
- Simplifies deployment and debugging
- No runtime dependency issues
- Container-friendly (small, static)

## Performance Targets

| Metric | Target |
|--------|--------|
| Search QPS (100K vectors) | >10,000 |
| Insert latency (p99) | <10ms |
| Recall@10 | >95% vs brute force |
| Election time | <500ms |
| Recovery (1M vectors) | <5s |

## Future Enhancements

- Product Quantization for memory efficiency
- Sharding for horizontal scaling
- Client-side load balancing
- Backup and restore tooling
