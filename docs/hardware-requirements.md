# Stacks Node Hardware Requirements

## Minimum Requirements

| Role | CPU | RAM | Disk | Network |
|------|-----|-----|------|---------|
| **Follower** | 4 cores | 8 GB | 200 GB NVMe¹ | 10 Mbps |
| **Miner** | 8 cores | 16 GB | 500 GB NVMe | 50 Mbps |
| **Signer** | 4 cores | 8 GB | 100 GB NVMe | 10 Mbps |

## Recommended

| Role | CPU | RAM | Disk | Network |
|------|-----|-----|------|---------|
| **Follower** | 8 cores | 32 GB | 500 GB NVMe | 100 Mbps |
| **Miner** | 16 cores | 32 GB | 1 TB NVMe | 100 Mbps |
| **Signer** | 8 cores | 16 GB | 200 GB NVMe | 50 Mbps |

¹ With pruned nodes (coming soon), follower disk requirements drop to ~110 GB.

## Additional Requirements

- **Bitcoin Core**: A Stacks node requires access to a Bitcoin node. Running your own adds ~700 GB disk (full) or ~5 GB (pruned with sufficient history). Alternatively, use a trusted Bitcoin RPC endpoint.
- **Operating System**: Linux (Ubuntu 22.04+ recommended). macOS supported for development. Windows not recommended for production.
- **File Descriptors**: Set `ulimit -n 524288` or configure `LimitNOFILE=524288` in systemd. The default limit of 1024 will cause crashes under load.

## Cloud Provider Estimates

| Provider | Server Type | Specs | Monthly Cost |
|----------|------------|-------|-------------|
| Hetzner | CPX41 | 8 vCPU, 16 GB, 240 GB | ~€15 |
| Hetzner | CPX62 | 16 vCPU, 32 GB, 640 GB | ~€45 |
| AWS | m6i.xlarge | 4 vCPU, 16 GB + 500 GB gp3 | ~$200 |
| AWS | m6i.2xlarge | 8 vCPU, 32 GB + 1 TB gp3 | ~$400 |

## Key Resource Consumers

- **MARF trie**: The largest disk consumer. Stores state trie with copy-on-write versioning.
- **SQLite databases**: Multiple databases (chainstate, sortition, clarity, mempool). WAL files can spike during heavy processing.
- **Block processor thread**: Uses a 32 MB stack. Deep smart contract execution may require this.
- **P2P connections**: Default allows up to 750 inbound + 1000 HTTP connections.
- **Prometheus metrics**: Minimal overhead when enabled (~5 MB RAM).

## Build Requirements

Building from source requires more resources than running:

| Profile | RAM Required | Disk | Time |
|---------|-------------|------|------|
| `cargo build --release` (fat LTO) | 16 GB+ | 10 GB | 15-30 min |
| `cargo build --profile release-lite` (thin LTO) | 8 GB+ | 10 GB | 10-20 min |
| `cargo build` (dev) | 8 GB+ | 10 GB | 5-10 min |
