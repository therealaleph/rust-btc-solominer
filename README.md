# Bitcoin Solo Miner - Rust Edition

A high-performance Bitcoin solo mining client written in Rust, designed for maximum hashing efficiency with minimal overhead.

**Credits**: x.com/hey_itsmyturn | t.me/itsthealephyouknowfromtwitter

## Overview

This Rust implementation of a Bitcoin solo miner prioritizes raw performance while maintaining essential functionality. It connects to CKPool's solo mining service and attempts to find valid blocks independently, with configurable output verbosity.

## Key Features

- **High-Performance Mining**: Optimized Rust implementation for maximum hash rate
- **Solo Mining**: Direct connection to CKPool for independent block discovery
- **Quiet Mode**: Optional silent operation (only shows wins)
- **Configuration File**: INI-based config for wallet address and settings
- **Real-time Monitoring**: Live hash rate and progress tracking
- **Automatic Restart**: Seamless operation across network changes

## Performance Optimizations

- **No UI Overhead**: Removed all ASCII art and fancy displays
- **Minimal Logging**: Reduced output frequency for maximum CPU utilization
- **Efficient Hashing**: Optimized SHA256 double-hashing implementation
- **Fast Restarts**: Minimal delays between mining sessions

## Prerequisites

- Rust 1.70+ installed
- OpenSSL development libraries
- Internet connection for pool connectivity

## Installation

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Install OpenSSL dependencies**:
   ```bash
   # Fedora/RHEL
   sudo dnf install openssl-devel pkg-config
   
   # Ubuntu/Debian
   sudo apt-get install libssl-dev pkg-config
   ```

3. **Build the miner**:
   ```bash
   cargo build --release
   ```

## Configuration

### Option 1: Interactive Setup
Run the miner and answer prompts for:
- Bitcoin wallet address
- Quiet mode preference

### Option 2: Configuration File
Create a `config.ini` file in the same directory:

```ini
[miner]
# Your Bitcoin wallet address for mining rewards
wallet_address = 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa

# Enable quiet mode (1 = enabled, 0 = disabled)
quiet_mode = 0
```

## Usage Instructions

### Basic Usage
```bash
./target/release/bitcoin-solo-miner
```

### With Pre-configured Address
```bash
echo "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa" | ./target/release/bitcoin-solo-miner
```

### Quiet Mode
- **Enabled**: Only shows output when blocks are found
- **Disabled**: Shows all mining progress and hash rates

## How It Works

1. **Connection**: Establishes connection to CKPool's solo mining service
2. **Authentication**: Authenticates with your Bitcoin address
3. **Job Retrieval**: Receives mining jobs with block parameters
4. **Hash Generation**: Generates SHA256 double-hashes with random nonces
5. **Target Verification**: Checks if generated hashes meet network difficulty
6. **Solution Submission**: Submits valid solutions to the pool

## Technical Architecture

- **Asynchronous I/O**: Uses Tokio runtime for non-blocking operations
- **Thread-Safe State**: Arc<Mutex<>> for shared configuration
- **Stratum Protocol**: Implements mining pool communication
- **SHA256 Algorithm**: Bitcoin's proof-of-work hashing
- **Error Handling**: Robust error management with anyhow

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

## Debug Information

Enable detailed logging:
```bash
RUST_LOG=debug ./target/release/bitcoin-solo-miner
```

## Important Notes

- **Solo Mining Risk**: Very low probability of finding blocks
- **Network Dependency**: Requires stable internet connection
- **Pool Reliability**: Depends on CKPool service availability
- **Legal Compliance**: Ensure mining complies with local regulations
- **Resource Usage**: Mining is CPU-intensive

## Support

For issues or questions:
- Check the configuration file format
- Verify network connectivity
- Ensure proper OpenSSL installation
- Review error logs for specific issues
