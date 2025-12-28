# Bitcoin Solo Miner - Rust Edition

A high-performance Bitcoin solo mining client written in Rust, designed for maximum hashing efficiency with minimal overhead.

**Credits**: [x.com/hey_itsmyturn](https://x.com/hey_itsmyturn) | <a href="https://t.me/itsthealephyouknowfromtwitter" target="_blank">Telegram</a> | <a href="https://sh1n.org" target="_blank">Website</a>

## Overview

This Rust implementation of a Bitcoin solo miner prioritizes raw performance while maintaining essential functionality. It connects to CKPool's solo mining service and attempts to find valid blocks independently, with configurable output verbosity and comprehensive error handling.

## Key Features

- **High-Performance Mining**: Optimized Rust implementation for maximum hash rate
- **Solo Mining**: Direct connection to CKPool for independent block discovery
- **Quiet Mode**: Optional silent operation (only shows wins)
- **Configuration Options**: Support for both config file and environment variables
- **Real-time Monitoring**: Live hash rate and progress tracking
- **Automatic Restart**: Seamless operation across network changes
- **Telegram Integration**: Optional notifications for startup and block discovery
- **Docker Support**: Full Docker Compose integration with automatic restart
- **Log Persistence**: Block discovery logs saved to file for permanent records

## Recent Updates

### Critical Bug Fixes
- Fixed block header format creation (removed invalid hardcoded padding)
- Fixed target calculation from nbits using proper Bitcoin compact format algorithm
- Fixed target comparison to use integer comparison instead of string comparison
- Fixed extranonce2 formatting for proper 8 hex character output

### Code Quality Improvements
- Replaced all unwrap() calls with proper error handling using anyhow
- Fixed race conditions in block height monitoring
- Improved hash rate calculation precision
- Added proper error handling for hex decoding operations
- Extracted magic numbers to named constants
- Optimized mutex lock granularity
- Added Bitcoin address format validation
- Added comprehensive documentation comments

### New Features
- Telegram integration for startup and block found notifications
- Environment variable support (BTC_ADDRESS, QUIET_MODE, TELEGRAM_BOT_TOKEN, TELEGRAM_USER_ID)
- Docker Compose support with restart policy and log persistence
- Block discovery logging to file for persistence
- Non-interactive mode support for Docker/CI environments

## Quick Start - One-Liner Deployment

**Deploy the Bitcoin solo miner with a single command. The script will prompt you to choose between local or remote deployment:**

```bash
bash <(curl -sSL https://raw.githubusercontent.com/therealaleph/rust-btc-solominer/main/deploy.sh)
```

**Remote Server Deployment:**

When you select "remote" deployment (default), the script will:
- Prompt for server connection details (IP, SSH username, authentication method)
- Test SSH connection before proceeding
- Prompt for Bitcoin address and optional Telegram credentials
- Automatically install Docker and dependencies on the remote server
- Clone the repository and deploy the miner container
- Display status and logs

**Requirements for remote deployment:**
- Ubuntu/Debian-based server
- SSH access (key-based or password authentication)
- Root or sudo privileges

**Local Deployment:**

When you select "local" deployment, the script will:
- Check for Docker and Docker Compose installation
- Prompt for Bitcoin address and optional Telegram credentials
- Build and start the miner container locally
- Display status and logs

**Requirements for local deployment:**
- Docker and Docker Compose installed
- Sufficient disk space for the Docker image (~1GB)

**Manual Deployment:**

Alternatively, clone the repository and run the deployment script manually:

```bash
git clone https://github.com/therealaleph/rust-btc-solominer.git
cd rust-btc-solominer
bash deploy.sh
```

## Usage Instructions

### Standard Usage (Built Binary)

```bash
./target/release/bitcoin-solo-miner
```

### Docker Usage (Manual)

```bash
# Set environment variables
export BTC_ADDRESS=your_bitcoin_address
export TELEGRAM_BOT_TOKEN=your_bot_token  # Optional
export TELEGRAM_USER_ID=your_user_id      # Optional

# Start miner
docker-compose up --build -d

# View logs
docker-compose logs -f

# Stop miner
docker-compose down
```

### Quiet Mode

- **Enabled**: Only shows output when blocks are found
- **Disabled**: Shows all mining progress and hash rates

## How It Works

1. **Connection**: Establishes connection to CKPool's solo mining service
2. **Authentication**: Authenticates with your Bitcoin address
3. **Job Retrieval**: Receives mining jobs with block parameters
4. **Hash Generation**: Generates SHA256 double-hashes with sequential nonces
5. **Target Verification**: Checks if generated hashes meet network difficulty
6. **Solution Submission**: Submits valid solutions to the pool

## Technical Architecture

- **Asynchronous I/O**: Uses Tokio runtime for non-blocking operations
- **Thread-Safe State**: Arc<Mutex<>> for shared configuration
- **Stratum Protocol**: Implements mining pool communication
- **SHA256 Algorithm**: Bitcoin's proof-of-work hashing
- **Error Handling**: Robust error management with anyhow
- **Block Header Format**: Properly formatted 80-byte Bitcoin block headers
- **Target Calculation**: Correct nbits to target conversion using compact format

## Docker Compose Features

- Automatic restart on failure (`restart: always`)
- Full logging to stdout and file
- Environment variable configuration
- Log persistence on host machine (`./logs/`)
- Block discovery logs saved to `./logs/blocks_found.log`
- Detached mode support

## Telegram Integration

To enable Telegram notifications:

1. Create a bot with [@BotFather](https://t.me/BotFather) on Telegram
2. Get your user ID from [@userinfobot](https://t.me/userinfobot)
3. Add both to config.ini or set as environment variables:
   ```bash
   export TELEGRAM_BOT_TOKEN=your_bot_token
   export TELEGRAM_USER_ID=your_user_id
   ```

Notifications are sent for:
- Miner startup
- Block discovery

## Expected Performance

- **Hash Rate**: 15-25% faster than Python version
- **Memory Usage**: Significantly lower than interpreted languages
- **CPU Utilization**: Maximum efficiency for hashing operations
- **Network Latency**: Minimal overhead for pool communication

## Dependencies

- **tokio**: Asynchronous runtime
- **serde/serde_json**: JSON serialization
- **reqwest**: HTTP client for APIs
- **sha2**: SHA256 hashing implementation
- **hex**: Hexadecimal encoding/decoding
- **rand**: Random number generation
- **log/env_logger**: Logging system
- **anyhow**: Error handling
- **configparser**: INI file parsing
- **atty**: Terminal detection for non-interactive mode

## Debug Information

Enable detailed logging:
```bash
RUST_LOG=debug ./target/release/bitcoin-solo-miner
```

Or with Docker:
```bash
RUST_LOG=debug docker-compose up
```

## Log Files

When running in Docker or with proper permissions, block discoveries are logged to:
- `./logs/blocks_found.log` - Persistent log of all block discoveries with timestamps

## Important Notes

- **Solo Mining Risk**: Very low probability of finding blocks
- **Network Dependency**: Requires stable internet connection
- **Pool Reliability**: Depends on CKPool service availability
- **Legal Compliance**: Ensure mining complies with local regulations
- **Resource Usage**: Mining is CPU-intensive
- **Block Discovery**: Extremely rare - requires astronomical luck for solo mining

## Troubleshooting

- **Connection Issues**: Verify network connectivity and pool availability
- **Build Errors**: Ensure OpenSSL development libraries are installed
- **Permission Errors**: Check file permissions for log directory
- **Address Validation**: Ensure Bitcoin address format is correct
- **Docker Issues**: Verify Docker daemon is running and environment variables are set

## License

See LICENSE file for details.
