# Vajra Operational Runbook

## Quick Reference

### Starting a Node

```bash
# With default configuration
./vajra

# With custom configuration
./vajra --config /path/to/vajra.toml

# With specific node ID and listen address
./vajra --node-id 1 --listen 0.0.0.0:50051

# With debug logging
./vajra --log-level debug
```

### Checking Node Health

```bash
# Metrics endpoint
curl http://localhost:9090/metrics

# Key metrics to check:
# - vajra_raft_is_leader: 1 if this node is leader
# - vajra_raft_term: Current Raft term
# - vajra_raft_commit_index: Committed log index
# - vajra_vectors_total: Number of vectors in index
```

### Graceful Shutdown

Send SIGTERM or press Ctrl+C. The node will:
1. Stop accepting new requests
2. Flush pending traces and metrics
3. Close all connections
4. Exit cleanly

## Cluster Operations

### Initial Cluster Bootstrap

TODO: To be documented in Phase 4

### Adding a Node

TODO: To be documented in Phase 4

### Removing a Node

TODO: To be documented in Phase 4

### Leader Failover

TODO: To be documented in Phase 4

## Troubleshooting

### Node Won't Start

1. **Check WAL corruption**: Look for `WAL_CORRUPTION` or `CHECKSUM_MISMATCH` in logs
2. **Check disk space**: WAL and snapshot directories need space
3. **Check permissions**: WAL directory must be writable
4. **Check port availability**: gRPC and metrics ports must be free

### Split Brain Suspected

1. Check `vajra_raft_is_leader` on all nodes
2. If multiple leaders, check network connectivity
3. "Not leader" errors indicate correct behavior

### High Search Latency

1. Check `vajra_search_duration_seconds` histogram
2. Check `ef_search` parameter - higher = slower but better recall
3. Check vector count vs memory available
4. Check CPU usage - searches are CPU-bound

### Memory Usage Growing

1. Check `vajra_vectors_deleted` - high count indicates pending compaction
2. Trigger snapshot to compact deleted vectors
3. Check for memory leaks in traces

## Configuration Reference

See `docs/ARCHITECTURE.md` for full configuration documentation.

### Critical Settings

| Setting | Default | Notes |
|---------|---------|-------|
| `raft.election_timeout_min_ms` | 150 | Must be > 2x network RTT |
| `raft.heartbeat_interval_ms` | 50 | Must be < election_timeout_min/3 |
| `storage.sync_policy` | batched | Use `every_entry` for max durability |
| `engine.ef_search` | 50 | Increase for better recall |

## Backup and Recovery

TODO: To be documented in Phase 5

## Monitoring Alerts

### Recommended Alerts

```yaml
# Leader missing for too long
- alert: VajraNoLeader
  expr: sum(vajra_raft_is_leader) == 0
  for: 30s
  
# Term increasing rapidly (election storms)
- alert: VajraElectionStorm
  expr: rate(vajra_raft_term[1m]) > 1
  
# High search latency
- alert: VajraHighSearchLatency
  expr: histogram_quantile(0.99, vajra_search_duration_seconds) > 0.1
  
# WAL growing too large
- alert: VajraWalTooLarge
  expr: vajra_wal_size_bytes > 1e9
```

## Contact

For issues, check the logs first. Enable debug logging with `--log-level debug`.
