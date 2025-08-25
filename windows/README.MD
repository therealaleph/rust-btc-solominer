# Bitcoin Solo Miner for Windows

High-performance Bitcoin solo mining client written in Rust.

## Credits
- x.com/hey_itsmyturn
- t.me/itsthealephyouknowfromtwitter

## Requirements
- Windows 10/11 (64-bit)
- No additional dependencies required

## Installation
1. Extract all files to a folder
2. Edit `config.ini` with your Bitcoin wallet address
3. Run `start-miner.bat` or `bitcoin-solo-miner.exe` directly

## Configuration
Edit `config.ini`:
```ini
[miner]
wallet_address=YOUR_BITCOIN_WALLET_ADDRESS_HERE
quiet_mode=false
```

## Usage
### Option 1: Interactive Mode
Double-click `bitcoin-solo-miner.exe` and follow prompts

### Option 2: Config File Mode
1. Set your wallet address in `config.ini`
2. Run `start-miner.bat` for automatic startup

### Option 3: Command Line
```cmd
bitcoin-solo-miner.exe
```

## Features
- High-performance SHA256 mining
- Automatic pool connection to CKPool
- Real-time hash rate monitoring
- Quiet mode for minimal output
- Automatic block height monitoring

## Performance
- Optimized for maximum CPU utilization
- Batch processing for efficiency
- Minimal restart delays

## Support
For issues or questions, check the main README.md file.
