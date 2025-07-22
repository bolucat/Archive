# Configuration Options

NodePass uses a minimalist approach to configuration, with all settings specified via command-line parameters and environment variables. This guide explains all available configuration options and provides recommendations for various deployment scenarios.

## Log Levels

NodePass provides six log verbosity levels that control the amount of information displayed:

- `none`: Disable logging - no log information displayed
- `debug`: Verbose debugging information - shows all operations and connections
- `info`: General operational information (default) - shows startup, shutdown, and key events
- `warn`: Warning conditions - only shows potential issues that don't affect core functionality
- `error`: Error conditions - shows only problems that affect functionality
- `event`: Event recording - shows important operational events and traffic statistics

You can set the log level in the command URL:

```bash
nodepass server://0.0.0.0:10101/0.0.0.0:8080?log=debug
```

## TLS Encryption Modes

For server and master modes, NodePass offers three TLS security levels for data channels:

- **Mode 0**: No TLS encryption (plain TCP/UDP)
  - Fastest performance, no overhead
  - No security for data channel (only use in trusted networks)
  
- **Mode 1**: Self-signed certificate (automatically generated)
  - Good security with minimal setup
  - Certificate is automatically generated and not verified
  - Protects against passive eavesdropping
  
- **Mode 2**: Custom certificate (requires `crt` and `key` parameters)
  - Highest security with certificate validation
  - Requires providing certificate and key files
  - Suitable for production environments

Example with TLS Mode 1 (self-signed):
```bash
nodepass server://0.0.0.0:10101/0.0.0.0:8080?tls=1
```

Example with TLS Mode 2 (custom certificate):
```bash
nodepass "server://0.0.0.0:10101/0.0.0.0:8080?tls=2&crt=/path/to/cert.pem&key=/path/to/key.pem"
```

## Connection Pool Capacity Parameters

Connection pool capacity can be configured via URL query parameters:

- `min`: Minimum connection pool capacity (default: 64)
- `max`: Maximum connection pool capacity (default: 1024)

Example:
```bash
# Set minimum pool to 32 and maximum to 4096
nodepass "client://server.example.com:10101/127.0.0.1:8080?min=32&max=4096"
```

## URL Query Parameter Scope and Applicability

NodePass allows flexible configuration via URL query parameters. The following table shows which parameters are applicable in server, client, and master modes:

| Parameter | Description           | server | client | master |
|-----------|----------------------|:------:|:------:|:------:|
| `log`     | Log level             |   O    |   O    |   O    |
| `tls`     | TLS encryption mode   |   O    |   X    |   O    |
| `crt`     | Custom certificate path|  O    |   X    |   O    |
| `key`     | Custom key path       |   O    |   X    |   O    |
| `min`     | Minimum pool capacity |   X    |   O    |   X    |
| `max`     | Maximum pool capacity |   O    |   O    |   X    |

- O: Parameter is valid and recommended for configuration
- X: Parameter is not applicable and should be ignored

**Best Practices:**
- For server/master modes, configure security-related parameters (`tls`, `crt`, `key`) to enhance data channel security.
- For client/master modes, adjust connection pool capacity (`min`, `max`) based on traffic and resource constraints for optimal performance.
- Log level (`log`) can be set in all modes for easier operations and troubleshooting.

## Environment Variables

NodePass behavior can be fine-tuned using environment variables. Below is the complete list of available variables with their descriptions, default values, and recommended settings for different scenarios.

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `NP_SEMAPHORE_LIMIT` | Maximum number of concurrent connections | 1024 | `export NP_SEMAPHORE_LIMIT=2048` |
| `NP_UDP_DATA_BUF_SIZE` | Buffer size for UDP packets | 8192 | `export NP_UDP_DATA_BUF_SIZE=16384` |
| `NP_UDP_READ_TIMEOUT` | Timeout for UDP read operations | 20s | `export NP_UDP_READ_TIMEOUT=30s` |
| `NP_UDP_DIAL_TIMEOUT` | Timeout for establishing UDP connections | 20s | `export NP_UDP_DIAL_TIMEOUT=30s` |
| `NP_TCP_READ_TIMEOUT` | Timeout for TCP read operations | 20s | `export NP_TCP_READ_TIMEOUT=30s` |
| `NP_TCP_DIAL_TIMEOUT` | Timeout for establishing TCP connections | 20s | `export NP_TCP_DIAL_TIMEOUT=30s` |
| `NP_MIN_POOL_INTERVAL` | Minimum interval between connection creations | 1s | `export NP_MIN_POOL_INTERVAL=500ms` |
| `NP_MAX_POOL_INTERVAL` | Maximum interval between connection creations | 5s | `export NP_MAX_POOL_INTERVAL=3s` |
| `NP_REPORT_INTERVAL` | Interval for health check reports | 5s | `export NP_REPORT_INTERVAL=10s` |
| `NP_SERVICE_COOLDOWN` | Cooldown period before restart attempts | 3s | `export NP_SERVICE_COOLDOWN=5s` |
| `NP_SHUTDOWN_TIMEOUT` | Timeout for graceful shutdown | 5s | `export NP_SHUTDOWN_TIMEOUT=10s` |
| `NP_RELOAD_INTERVAL` | Interval for cert/pool reload | 1h | `export NP_RELOAD_INTERVAL=30m` |

### Connection Pool Tuning

The connection pool parameters are important settings for performance tuning:

#### Pool Capacity Settings

- `min` (URL parameter): Ensures a minimum number of available connections
  - Too low: Increased latency during traffic spikes as new connections must be established
  - Too high: Wasted resources maintaining idle connections
  - Recommended starting point: 25-50% of your average concurrent connections

- `max` (URL parameter): Prevents excessive resource consumption while handling peak loads
  - Too low: Connection failures during traffic spikes
  - Too high: Potential resource exhaustion affecting system stability
  - Recommended starting point: 150-200% of your peak concurrent connections

#### Pool Interval Settings

- `NP_MIN_POOL_INTERVAL`: Controls the minimum time between connection creation attempts
  - Too low: May overwhelm network with connection attempts
  - Recommended range: 500ms-2s depending on network latency

- `NP_MAX_POOL_INTERVAL`: Controls the maximum time between connection creation attempts
  - Too high: May result in pool depletion during traffic spikes
  - Recommended range: 3s-10s depending on expected traffic patterns

#### Connection Management

- `NP_SEMAPHORE_LIMIT`: Controls the maximum number of concurrent tunnel operations
  - Too low: Rejected connections during traffic spikes
  - Too high: Potential memory pressure from too many concurrent goroutines
  - Recommended range: 1000-5000 for most applications, higher for high-throughput scenarios

### UDP Settings

For applications relying heavily on UDP traffic:

- `NP_UDP_DATA_BUF_SIZE`: Buffer size for UDP packets
  - Increase for applications sending large UDP packets
  - Default (8192) works well for most cases
  - Consider increasing to 16384 or higher for media streaming or game servers

- `NP_UDP_READ_TIMEOUT`: Timeout for UDP read operations
  - Increase for high-latency networks or applications with slow response times
  - Decrease for low-latency applications requiring quick failover

- `NP_UDP_DIAL_TIMEOUT`: Timeout for establishing UDP connections
  - Increase for high-latency networks or applications with slow response times
  - Decrease for low-latency applications requiring quick failover

### TCP Settings

For optimizing TCP connections:

- `NP_TCP_READ_TIMEOUT`: Timeout for TCP read operations
  - Increase for high-latency networks or servers with slow response times
  - Decrease for applications that need to detect disconnections quickly
  - Affects wait time during data transfer phases

- `NP_TCP_DIAL_TIMEOUT`: Timeout for establishing TCP connections
  - Increase for unstable network conditions
  - Decrease for applications that need quick connection success/failure determination
  - Affects initial connection establishment phase

### Service Management Settings

- `NP_REPORT_INTERVAL`: Controls how frequently health status is reported
  - Lower values provide more frequent updates but increase log volume
  - Higher values reduce log output but provide less immediate visibility

- `NP_RELOAD_INTERVAL`: Controls how frequently TLS certificates are checked for changes
  - Lower values detect certificate changes faster but increase file system operations
  - Higher values reduce overhead but delay detection of certificate updates

- `NP_SERVICE_COOLDOWN`: Time to wait before attempting service restarts
  - Lower values attempt recovery faster but might cause thrashing in case of persistent issues
  - Higher values provide more stability but slower recovery from transient issues

- `NP_SHUTDOWN_TIMEOUT`: Maximum time to wait for connections to close during shutdown
  - Lower values ensure quicker shutdown but may interrupt active connections
  - Higher values allow more time for connections to complete but delay shutdown

## Configuration Profiles

Here are some recommended environment variable configurations for common scenarios:

### High-Throughput Configuration

For applications requiring maximum throughput (e.g., media streaming, file transfers):

URL parameters:
```bash
nodepass "client://server.example.com:10101/127.0.0.1:8080?min=128&max=8192"
```

Environment variables:
```bash
export NP_MIN_POOL_INTERVAL=500ms
export NP_MAX_POOL_INTERVAL=3s
export NP_SEMAPHORE_LIMIT=8192
export NP_UDP_DATA_BUF_SIZE=32768
export NP_REPORT_INTERVAL=10s
```

### Low-Latency Configuration

For applications requiring minimal latency (e.g., gaming, financial trading):

URL parameters:
```bash
nodepass "client://server.example.com:10101/127.0.0.1:8080?min=256&max=4096"
```

Environment variables:
```bash
export NP_MIN_POOL_INTERVAL=100ms
export NP_MAX_POOL_INTERVAL=1s
export NP_SEMAPHORE_LIMIT=4096
export NP_UDP_READ_TIMEOUT=5s
export NP_REPORT_INTERVAL=1s
```

### Resource-Constrained Configuration

For deployment on systems with limited resources (e.g., IoT devices, small VPS):

URL parameters:
```bash
nodepass "client://server.example.com:10101/127.0.0.1:8080?min=16&max=512"
```

Environment variables:
```bash
export NP_MIN_POOL_INTERVAL=2s
export NP_MAX_POOL_INTERVAL=10s
export NP_SEMAPHORE_LIMIT=512
export NP_REPORT_INTERVAL=30s
export NP_SHUTDOWN_TIMEOUT=3s
```

## Next Steps

- See [usage instructions](/docs/en/usage.md) for basic operational commands
- Explore [examples](/docs/en/examples.md) to understand deployment patterns
- Learn about [how NodePass works](/docs/en/how-it-works.md) to optimize your configuration
- Check the [troubleshooting guide](/docs/en/troubleshooting.md) if you encounter issues