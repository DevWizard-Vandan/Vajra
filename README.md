# Vajra ⚡

## 🌐 [Live Demo → devwizard-vandan.github.io/Vajra](https://devwizard-vandan.github.io/Vajra)
## 🔗 [Part of the HFT Stack → devwizard-vandan.github.io/HFT-Stack](https://devwizard-vandan.github.io/HFT-Stack/)
A distributed, fault-tolerant, in-memory vector database built from first principles in Rust.

[![Demo](https://img.shields.io/badge/demo-live-brightgreen)](https://devwizard-vandan.github.io/Vajra)
[![Tests](https://img.shields.io/badge/tests-110%20passing-brightgreen)](https://github.com/DevWizard-Vandan/Vajra)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)

---

## Why I built this

I built Vajra strictly out of curiosity and a deep desire to learn systems programming from the ground up. I wanted to demystify the magic behind distributed databases and modern AI infrastructure. Instead of just using existing tools like Milvus or Qdrant, I wanted to understand the raw mechanics of consensus algorithms, write-ahead logs, and vector indexes. Rust provided the perfect playground—demanding rigor while empowering performance. What started as late-night tinkering with Raft and HNSW eventually became this project; a testament to breaking down black boxes and building them back up from first principles.

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

### Component Roles

| Component | Responsibility |
|-----------|----------------|
| **gRPC Server** | Primary API gateway, peer-to-peer sync |
| **REST API** | Lightweight HTTP gateway (`/upsert`, `/search`, `/health`) |
| **Reactor** | Event loop with biased priority |
| **Raft** | Consensus, leader election, log replication |
| **WAL** | Crash recovery, durability |
| **HNSW** | In-memory vector search |
| **Transport** | Peer-to-peer gRPC communication |

---

## Raft State Machine

### States

```
                    timeout
    ┌────────────────────────────────┐
    │                                │
    ▼                                │
┌────────┐  receive higher term  ┌───┴─────┐  win election  ┌────────┐
│Follower│◀─────────────────────│Candidate│───────────────▶│ Leader │
└────────┘                       └─────────┘                └────────┘
    ▲                                                            │
    │                    discover higher term                    │
    └────────────────────────────────────────────────────────────┘
```

### Key Invariants

1. **Election Safety**: At most one leader per term
2. **Leader Append-Only**: Leader never overwrites its log
3. **Log Matching**: If two logs have same index+term, all prior entries match
4. **State Machine Safety**: If entry applied, no other entry at same index

### Pre-Vote Extension

Prevents term inflation during network partitions:
```rust
// Before starting election, check if we CAN win
if !pre_vote_granted_by_majority() {
    return; // Don't increment term, stay follower
}
start_real_election();
```

---

## HNSW Design Decisions

### Why HNSW over alternatives?

| Algorithm | Build Time | Query Time | Memory | Dynamic |
|-----------|------------|------------|--------|---------|
| Brute Force | O(1) | O(N) | O(N) | ✅ |
| KD-Tree | O(N log N) | O(N^0.7) | O(N) | ❌ |
| LSH | O(N) | O(1)* | O(N) | ⚠️ |
| **HNSW** | O(N log N) | **O(log N)** | O(N) | ✅ |

### Tradeoffs Made

**1. Memory over Disk**
- HNSW graph lives entirely in RAM
- Tradeoff: Fast search, but limited by memory
- Mitigation: WAL ensures durability

**2. Approximate over Exact**
- `ef_search` controls accuracy/speed tradeoff
- Higher `ef` = better recall, slower search
- Default: 95%+ recall at sub-ms latency

**3. M Parameter Selection**
```rust
M = 16      // Connections per node
// Higher M = better recall, more memory
// Lower M = less memory, worse recall
// 16 is sweet spot for 128-dim vectors
```

---

## Failure Scenarios Tested

### 1. Leader Crash
```
Scenario: Leader dies mid-heartbeat
Expected: New leader elected in <300ms
Tested:   ✅ Election timeout triggers, new leader elected
```

### 2. Network Partition
```
Scenario: Leader isolated from majority
Expected: Old leader steps down, new leader on majority side
Tested:   ✅ Pre-Vote prevents term inflation
```

### 3. Split Brain Prevention
```
Scenario: Network heals after partition
Expected: Single leader, consistent log
Tested:   ✅ Higher term leader wins, logs reconcile
```

### 4. WAL Corruption
```
Scenario: Crash during write, partial entry
Expected: CRC32 detects corruption, truncate and recover
Tested:   ✅ Corrupt tail recovery in test suite
```

### 5. Follower Lag
```
Scenario: Follower falls behind leader
Expected: Leader sends missing entries via AppendEntries
Tested:   ✅ nextIndex[] tracks per-follower progress
```

---

## Why Biased Reactor?

### The Problem

```rust
// WRONG: Fair scheduling
tokio::select! {
    _ = ticker.tick() => { /* heartbeat */ }
    msg = client_rx.recv() => { /* client request */ }
}
```

If a search takes 50ms and heartbeat interval is 50ms, **heartbeats get delayed**. Followers think leader is dead. Unnecessary elections occur.

### The Solution

```rust
// RIGHT: Biased scheduling (heartbeats first)
tokio::select! {
    biased;  // <-- This changes everything
    
    _ = &mut shutdown_rx => { break; }      // 0. Shutdown
    _ = ticker.tick() => { raft.tick(); }   // 1. Heartbeats FIRST
    msg = client_rx.recv() => { ... }       // 2. Client requests
}
```

**Result**: Heartbeats are never delayed by slow searches. Cluster stability preserved.

> *"The heartbeat must never wait for a search."*

---

## Quick Start

```bash
# Build
cargo build --release

# Run single node
./target/release/vajra --node-id 1 --listen 127.0.0.1:50051

# Run 3-node cluster
.\scripts\trinity_demo.ps1 -Clean
```

---

## REST API Examples

You can interact with Vajra via the native gRPC layer, or through the lightweight Axum REST API (defaults to port `8080`).

```bash
# Check node health and Raft state
curl http://localhost:8080/health

# Insert a vector
curl -X POST http://localhost:8080/upsert \
  -H "Content-Type: application/json" \
  -d '{"id": "doc1", "vector": [0.1, 0.2, 0.3, 0.4]}'

# Search for nearest neighbors
curl -X POST http://localhost:8080/search \
  -H "Content-Type: application/json" \
  -d '{"query": [0.1, 0.2, 0.3, 0.4], "k": 10, "ef": 50}'
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

## License

MIT OR Apache-2.0
