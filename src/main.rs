use std::cmp::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::io::Write;
use std::fs::OpenOptions;
use std::path::Path;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::{json, Value};
use sha2::{Sha256, Digest};
use rand::Rng;
use log::{info, error, warn};
use anyhow::{Result, Context, bail};
use configparser::ini::Ini;

const CREDITS: &str = r#"
Bitcoin Solo Miner - Rust Edition
Credits: x.com/hey_itsmyturn | t.me/itsthealephyouknowfromtwitter
"#;

// Constants
const POOL_ADDRESS: &str = "solo.ckpool.org:3333";
const BLOCKCHAIN_API: &str = "https://blockchain.info/latestblock";
const TELEGRAM_API: &str = "https://api.telegram.org/bot";
const HASHES_PER_BATCH: u32 = 1000;
const HASH_RATE_LOG_INTERVAL_SECS: u64 = 5;
const BLOCK_HEIGHT_CHECK_INTERVAL_SECS: u64 = 40;
const MINING_RESTART_DELAY_MS: u64 = 100;
const BUFFER_SIZE: usize = 4096;
const EXTRANONCE2_SIZE_BYTES: usize = 4; // 4 bytes = 8 hex characters

#[derive(Debug, Clone)]
struct TelegramConfig {
    bot_token: String,
    user_id: String,
}

impl TelegramConfig {
    fn is_configured(&self) -> bool {
        !self.bot_token.is_empty() && !self.user_id.is_empty()
    }
}

#[derive(Debug)]
struct MiningJob {
    job_id: String,
    prevhash: String,
    coinb1: String,
    coinb2: String,
    merkle_branch: Vec<String>,
    version: String,
    nbits: String,
    ntime: String,
    clean_jobs: bool,
}

#[derive(Debug)]
struct MiningConfig {
    address: String,
    current_height: u64,
    quiet_mode: bool,
    telegram: Option<TelegramConfig>,
}

impl MiningConfig {
    fn new(address: String, quiet_mode: bool, telegram: Option<TelegramConfig>) -> Self {
        Self {
            address,
            current_height: 0,
            quiet_mode,
            telegram,
        }
    }
}

/// Load configuration from environment variables and config.ini file
/// Environment variables take precedence over config file
fn load_config() -> Result<(String, bool, Option<TelegramConfig>)> {
    // Check environment variables first (take precedence)
    let env_address = std::env::var("BTC_ADDRESS").ok();
    let env_quiet_mode = std::env::var("QUIET_MODE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .map(|v| v == 1);
    let env_telegram_token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
    let env_telegram_user_id = std::env::var("TELEGRAM_USER_ID").ok();
    
    // Load from config file if it exists (optional)
    let mut config = Ini::new();
    let mut address = String::new();
    let mut quiet_mode = false;
    let mut telegram_token = String::new();
    let mut telegram_user_id = String::new();
    
    // Try to load config.ini, but it's optional
    if Path::new("config.ini").exists() {
        if config.load("config.ini").is_ok() {
            address = config.get("miner", "wallet_address")
                .unwrap_or_else(|| "".to_string());
            quiet_mode = match config.getuint("miner", "quiet_mode") {
                Ok(Some(value)) => value == 1,
                _ => false
            };
            telegram_token = config.get("telegram", "bot_token")
                .unwrap_or_else(|| "".to_string());
            telegram_user_id = config.get("telegram", "user_id")
                .unwrap_or_else(|| "".to_string());
        }
    }
    
    // Override with environment variables if provided
    if let Some(env_addr) = env_address {
        address = env_addr;
    }
    
    if let Some(env_quiet) = env_quiet_mode {
        quiet_mode = env_quiet;
    }
    
    if let Some(env_token) = env_telegram_token {
        telegram_token = env_token;
    }
    
    if let Some(env_user_id) = env_telegram_user_id {
        telegram_user_id = env_user_id;
    }
    
    // Create telegram config if both token and user_id are available
    let telegram = if !telegram_token.is_empty() && !telegram_user_id.is_empty() {
        Some(TelegramConfig {
            bot_token: telegram_token,
            user_id: telegram_user_id,
        })
    } else {
        None
    };
    
    Ok((address, quiet_mode, telegram))
}

/// Log block found information to file
fn log_block_found(block_info: &str) -> Result<()> {
    let logs_dir = Path::new("/app/logs");
    if !logs_dir.exists() {
        std::fs::create_dir_all(logs_dir)?;
    }
    
    let log_file = logs_dir.join("blocks_found.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;
    
    writeln!(file, "{}", block_info)?;
    writeln!(file, "{}", "=".repeat(80))?;
    file.flush()?;
    
    info!("Block logged to: {}", log_file.display());
    Ok(())
}

/// Validate Bitcoin address format (basic check)
fn validate_bitcoin_address(address: &str) -> bool {
    // Basic validation: should be between 26-35 characters and alphanumeric (excluding ambiguous chars)
    if address.len() < 26 || address.len() > 35 {
        return false;
    }
    // Check for valid base58 characters (simplified check - alphanumeric but not 0, O, I, l)
    address.chars().all(|c| {
        c.is_alphanumeric() && c != '0' && c != 'O' && c != 'I' && c != 'l'
    })
}

/// Send Telegram message
async fn send_telegram_message(telegram: &TelegramConfig, message: &str) -> Result<()> {
    if !telegram.is_configured() {
        return Ok(());
    }
    
    let url = format!("{}{}/sendMessage", TELEGRAM_API, telegram.bot_token);
    let payload = json!({
        "chat_id": telegram.user_id,
        "text": message,
        "parse_mode": "HTML"
    });
    
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("Failed to send Telegram message")?;
    
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        warn!("Telegram API error: {} - {}", status, text);
    }
    
    Ok(())
}

/// Get current Bitcoin blockchain height
async fn get_current_block_height() -> Result<u64> {
    let response = reqwest::get(BLOCKCHAIN_API)
        .await
        .context("Failed to fetch blockchain height")?;
    let data: Value = response.json().await?;
    Ok(data["height"].as_u64().unwrap_or(0))
}

/// Double SHA256 hash
fn double_sha256(data: &[u8]) -> Vec<u8> {
    let first_hash = Sha256::digest(data);
    let second_hash = Sha256::digest(&first_hash);
    second_hash.to_vec()
}

/// Reverse hex string (byte-level reversal for little-endian)
fn reverse_hex_bytes(hex_str: &str) -> String {
    let mut reversed = String::new();
    for i in (0..hex_str.len()).step_by(2).rev() {
        if i + 1 < hex_str.len() {
            reversed.push_str(&hex_str[i..i+2]);
        }
    }
    reversed
}

/// Create Bitcoin block header (exactly 80 bytes / 160 hex chars)
/// Format: version(4) + prevhash(32) + merkle_root(32) + nbits(4) + ntime(4) + nonce(4)
fn create_block_header(
    version: &str,
    prevhash: &str,
    merkle_root: &str,
    nbits: &str,
    ntime: &str,
    nonce: &str,
) -> Result<Vec<u8>> {
    // Ensure all inputs are properly formatted (pad to expected lengths)
    let version_padded = format!("{:0>8}", version);
    let prevhash_padded = format!("{:0<64}", prevhash);
    let merkle_root_padded = format!("{:0<64}", merkle_root);
    let nbits_padded = format!("{:0>8}", nbits);
    let ntime_padded = format!("{:0>8}", ntime);
    let nonce_padded = format!("{:0>8}", nonce);
    
    // Combine all parts (160 hex characters = 80 bytes)
    let header_hex = format!(
        "{}{}{}{}{}{}",
        version_padded, prevhash_padded, merkle_root_padded, 
        nbits_padded, ntime_padded, nonce_padded
    );
    
    // Convert hex to bytes
    hex::decode(&header_hex)
        .context("Failed to decode block header hex")
        .map_err(|e| anyhow::anyhow!("Invalid block header format: {}", e))
}

/// Calculate target from nbits (Bitcoin compact format)
/// nbits format: first byte = exponent, next 3 bytes = mantissa
/// Target = mantissa * 256^(exponent - 3)
/// Returns target as 32-byte big-endian array for comparison
fn calculate_target(nbits: &str) -> Result<Vec<u8>> {
    if nbits.len() != 8 {
        bail!("nbits must be 8 hex characters (4 bytes)");
    }
    
    let nbits_bytes = hex::decode(nbits)
        .context("Failed to decode nbits")?;
    
    if nbits_bytes.len() != 4 {
        bail!("nbits must be 4 bytes");
    }
    
    let exponent = nbits_bytes[0] as u32;
    
    if exponent < 3 {
        bail!("Invalid nbits: exponent too small");
    }
    
    if exponent > 32 {
        bail!("Invalid nbits: exponent too large");
    }
    
    // Calculate target: mantissa * 256^(exponent - 3)
    // Target is stored as 32-byte big-endian number
    let mut target = vec![0u8; 32];
    
    // Mantissa is the 3 bytes after the exponent byte
    let mantissa_byte1 = nbits_bytes[1];
    let mantissa_byte2 = nbits_bytes[2];
    let mantissa_byte3 = nbits_bytes[3];
    
    // Calculate byte position for mantissa: (32 - exponent)
    // This positions the 3-byte mantissa at the correct location
    let shift_bytes = (exponent - 3) as usize;
    
    if shift_bytes >= 32 {
        // Target would overflow 32 bytes, return zero target
        return Ok(target);
    }
    
    // Place the 3 mantissa bytes starting at position (32 - shift_bytes - 3)
    let start_pos = 32_usize.saturating_sub(shift_bytes).saturating_sub(3);
    
    if start_pos < 32 {
        target[start_pos] = mantissa_byte1;
        if start_pos + 1 < 32 {
            target[start_pos + 1] = mantissa_byte2;
        }
        if start_pos + 2 < 32 {
            target[start_pos + 2] = mantissa_byte3;
        }
    }
    
    Ok(target)
}

/// Compare hash with target (both as byte arrays, big-endian)
fn hash_meets_target(hash: &[u8], target: &[u8]) -> bool {
    if hash.len() != 32 || target.len() != 32 {
        return false;
    }
    
    // Compare byte by byte (big-endian)
    for i in 0..32 {
        match hash[i].cmp(&target[i]) {
            Ordering::Less => return true,
            Ordering::Greater => return false,
            Ordering::Equal => continue,
        }
    }
    
    // Equal means hash meets target (<=)
    true
}

/// Bitcoin mining function
async fn bitcoin_miner(config: Arc<Mutex<MiningConfig>>) -> Result<()> {
    let (quiet_mode, address) = {
        let config_guard = config.lock().unwrap();
        (config_guard.quiet_mode, config_guard.address.clone())
    };

    if !quiet_mode {
        info!("Mining operation initiated");
        println!("[*] Connecting to {}...", POOL_ADDRESS);
    }
    
    let mut stream = TcpStream::connect(POOL_ADDRESS).await?;
    if !quiet_mode {
        println!("[*] Connected to mining pool");
    }
    
    // Subscribe to mining notifications
    let subscribe_msg = json!({
        "id": 1,
        "method": "mining.subscribe",
        "params": []
    });
    stream.write_all(format!("{}\n", subscribe_msg).as_bytes()).await?;
    if !quiet_mode {
        println!("[*] Subscribing to mining notifications...");
    }

    let mut buffer = vec![0u8; BUFFER_SIZE];
    let n = stream.read(&mut buffer).await?;
    let response = String::from_utf8_lossy(&buffer[..n]);
    
    let lines: Vec<&str> = response.split('\n').collect();
    let response_data: Value = serde_json::from_str(
        lines.first().context("Empty response from pool")?
    )?;
    
    let result = &response_data["result"];
    let extranonce1 = result[1].as_str()
        .context("Missing extranonce1 in subscribe response")?;
    let _extranonce2_size = result[2].as_u64().unwrap_or(0);

    if !quiet_mode {
        println!("[*] Subscription successful");
    }

    // Authorize with pool
    let authorize_msg = json!({
        "params": [address.clone(), "password"],
        "id": 2,
        "method": "mining.authorize"
    });
    stream.write_all(format!("{}\n", authorize_msg).as_bytes()).await?;

    if !quiet_mode {
        println!("[*] Waiting for mining job...");
    }
    
    // Read until we get a mining.notify message
    let mut response_data = String::new();
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            bail!("Connection closed by pool");
        }
        response_data.push_str(&String::from_utf8_lossy(&buffer[..n]));
        if response_data.contains("mining.notify") {
            break;
        }
    }

    let lines: Vec<&str> = response_data.split('\n').collect();
    let job_line = lines.iter()
        .find(|line| line.contains("mining.notify"))
        .context("No mining.notify message received")?;
    let job_data: Value = serde_json::from_str(job_line)?;
    let params = &job_data["params"];

    if params.as_array().map(|a| a.len()).unwrap_or(0) < 9 {
        bail!("Invalid mining.notify message: insufficient parameters");
    }

    let mining_job = MiningJob {
        job_id: params[0].as_str().context("Missing job_id")?.to_string(),
        prevhash: params[1].as_str().context("Missing prevhash")?.to_string(),
        coinb1: params[2].as_str().context("Missing coinb1")?.to_string(),
        coinb2: params[3].as_str().context("Missing coinb2")?.to_string(),
        merkle_branch: params[4].as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect(),
        version: params[5].as_str().context("Missing version")?.to_string(),
        nbits: params[6].as_str().context("Missing nbits")?.to_string(),
        ntime: params[7].as_str().context("Missing ntime")?.to_string(),
        clean_jobs: params[8].as_bool().unwrap_or(false),
    };

    let target = calculate_target(&mining_job.nbits)
        .context("Failed to calculate target from nbits")?;
    
    let mut rng = rand::thread_rng();
    let extranonce2_bytes: [u8; EXTRANONCE2_SIZE_BYTES] = rng.gen();
    let extranonce2 = format!("{:0>8}", hex::encode(extranonce2_bytes));

    // Build coinbase transaction
    let coinbase_hex = format!("{}{}{}{}", 
        mining_job.coinb1, extranonce1, extranonce2, mining_job.coinb2);
    
    let coinbase_bytes = hex::decode(&coinbase_hex)
        .context("Failed to decode coinbase hex")?;
    let coinbase_hash = double_sha256(&coinbase_bytes);
    let coinbase_hash_bin = coinbase_hash;

    // Calculate merkle root
    let mut merkle_root = coinbase_hash_bin;
    for branch in &mining_job.merkle_branch {
        let branch_bytes = hex::decode(branch)
            .context("Failed to decode merkle branch")?;
        let mut combined = merkle_root.clone();
        combined.extend_from_slice(&branch_bytes);
        merkle_root = double_sha256(&combined);
    }

    let merkle_root_hex = reverse_hex_bytes(&hex::encode(&merkle_root));
    
    // Get initial block height
    let initial_height = get_current_block_height().await?;
    let work_on = initial_height;
    
    if !quiet_mode {
        println!("[*] Working on network block height: {}", work_on);
        println!("[*] Starting hash generation...");
    }
    
    let mut hash_count = 0u64;
    let mut last_log_time = std::time::Instant::now();
    let mut nonce_counter: u32 = 0;
    
    loop {
        // Check if new block was found
        let current_height = {
            let config_guard = config.lock().unwrap();
            config_guard.current_height
        };
        
        if current_height > work_on {
            if !quiet_mode {
                println!("[*] New block detected, restarting mining operation");
            }
            break;
        }

        // Mining loop - try nonces
        for _ in 0..HASHES_PER_BATCH {
            // Use sequential nonce for better performance
            nonce_counter = nonce_counter.wrapping_add(1);
            let nonce_hex = format!("{:08x}", nonce_counter);
            
            let header_bytes = create_block_header(
                &mining_job.version,
                &mining_job.prevhash,
                &merkle_root_hex,
                &mining_job.nbits,
                &mining_job.ntime,
                &nonce_hex,
            ).context("Failed to create block header")?;

            let hash_bytes = double_sha256(&header_bytes);
            hash_count += 1;

            // Check if hash meets target
            if hash_meets_target(&hash_bytes, &target) {
                let hash_hex = hex::encode(&hash_bytes);
                let target_hex = hex::encode(&target);
                
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                let block_info = format!(
                    "[!] VALID BLOCK HASH DISCOVERED!\n\
                    [*] Hash: {}\n\
                    [*] Target: {}\n\
                    [*] Nonce: {}\n\
                    [*] Address: {}\n\
                    [*] Timestamp: {}\n",
                    hash_hex, target_hex, nonce_hex, address, timestamp
                );
                
                println!("{}", block_info);
                
                // Log to file
                if let Err(e) = log_block_found(&block_info) {
                    warn!("Failed to log block to file: {}", e);
                }

                // Send Telegram notification
                {
                    let config_guard = config.lock().unwrap();
                    if let Some(ref telegram) = config_guard.telegram {
                        let message = format!(
                            "🎉 <b>BLOCK FOUND!</b>\n\n\
                            Hash: <code>{}</code>\n\
                            Target: <code>{}</code>\n\
                            Nonce: <code>{}</code>\n\
                            Address: <code>{}</code>",
                            hash_hex, target_hex, nonce_hex, address
                        );
                        if let Err(e) = send_telegram_message(telegram, &message).await {
                            warn!("Failed to send Telegram notification: {}", e);
                        }
                    }
                }

                // Submit solution to pool
                let submit_msg = json!({
                    "params": [
                        address,
                        mining_job.job_id,
                        extranonce2,
                        mining_job.ntime,
                        nonce_hex
                    ],
                    "id": 1,
                    "method": "mining.submit"
                });

                stream.write_all(format!("{}\n", submit_msg).as_bytes()).await?;
                println!("[*] Solution submitted to pool");
                
                let mut response_buffer = vec![0u8; BUFFER_SIZE];
                let n = stream.read(&mut response_buffer).await?;
                let response = String::from_utf8_lossy(&response_buffer[..n]);
                println!("[*] Pool response: {}", response);

                return Ok(());
            }
        }

        // Log hash rate periodically
        if !quiet_mode {
            let elapsed = last_log_time.elapsed();
            if elapsed.as_secs() >= HASH_RATE_LOG_INTERVAL_SECS {
                let elapsed_secs = elapsed.as_secs_f64();
                let hash_rate = (hash_count as f64 / elapsed_secs) as u64;
                println!("[*] Hash rate: {} h/s | Total hashes: {}", hash_rate, hash_count);
                hash_count = 0;
                last_log_time = std::time::Instant::now();
            }
        }
    }

    Ok(())
}

/// Monitor for new blocks on the network
async fn new_block_listener(config: Arc<Mutex<MiningConfig>>) -> Result<()> {
    loop {
        let current_height = {
            let config_guard = config.lock().unwrap();
            config_guard.current_height
        };
        
        match get_current_block_height().await {
            Ok(network_height) => {
                if network_height > current_height {
                    let mut local_config = config.lock().unwrap();
                    if !local_config.quiet_mode {
                        info!("Network block height updated to {}", network_height);
                    }
                    local_config.current_height = network_height;
                }
            }
            Err(e) => {
                error!("Failed to fetch network block height: {}", e);
            }
        }
        
        tokio::time::sleep(Duration::from_secs(BLOCK_HEIGHT_CHECK_INTERVAL_SECS)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("{}", CREDITS);

    let (config_address, config_quiet, telegram_config) = load_config()?;
    
    // Get Bitcoin address - check env var, then config, then prompt
    let address = if !config_address.is_empty() {
        if !validate_bitcoin_address(&config_address) {
            warn!("Warning: Bitcoin address format may be invalid: {}", config_address);
        }
        config_address
    } else {
        // Check if running in Docker/non-interactive mode
        if !atty::is(atty::Stream::Stdin) {
            bail!("Bitcoin address is required. Set BTC_ADDRESS environment variable or configure in config.ini");
        }
        let mut input = String::new();
        print!("Enter your Bitcoin wallet address for mining rewards: ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut input)?;
        let addr = input.trim().to_string();
        if !validate_bitcoin_address(&addr) {
            warn!("Warning: Bitcoin address format may be invalid: {}", addr);
        }
        addr
    };

    // Get quiet mode preference - use config if available, otherwise check if non-interactive
    let quiet_mode = if !atty::is(atty::Stream::Stdin) {
        config_quiet
    } else {
        let mut quiet_input = String::new();
        print!("Enable quiet mode? (y/n) [default: n]: ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut quiet_input)?;
        config_quiet || quiet_input.trim().to_lowercase() == "y"
    };

    if !quiet_mode {
        println!("Bitcoin address: {}", address);
        println!("Quiet mode: {}", if quiet_mode { "enabled" } else { "disabled" });
        if telegram_config.is_some() {
            println!("Telegram notifications: enabled");
        } else {
            println!("Telegram notifications: disabled");
        }
        println!("Starting miner...");
    }

    let config = Arc::new(Mutex::new(MiningConfig::new(
        address.clone(),
        quiet_mode,
        telegram_config.clone(),
    )));

    // Send startup Telegram notification
    if let Some(ref telegram) = &telegram_config {
        let startup_message = format!(
            "🚀 <b>Bitcoin Solo Miner Started</b>\n\n\
            Address: <code>{}</code>\n\
            Quiet mode: {}\n\
            Pool: <code>{}</code>",
            address,
            if quiet_mode { "Yes" } else { "No" },
            POOL_ADDRESS
        );
        if let Err(e) = send_telegram_message(telegram, &startup_message).await {
            warn!("Failed to send startup Telegram notification: {}", e);
        }
    }

    // Spawn block height monitor
    let config_clone = Arc::clone(&config);
    let _listener_handle = tokio::spawn(async move {
        if let Err(e) = new_block_listener(config_clone).await {
            error!("Block monitoring error: {}", e);
        }
    });

    // Main mining loop
    loop {
        let config_clone = Arc::clone(&config);
        if let Err(e) = bitcoin_miner(config_clone).await {
            error!("Mining operation error: {}", e);
            tokio::time::sleep(Duration::from_millis(MINING_RESTART_DELAY_MS)).await;
        }
    }
}
