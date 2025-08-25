use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::io::Write;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::{json, Value};
use sha2::{Sha256, Digest};
use rand::Rng;
use log::{info, error};
use anyhow::Result;
use configparser::ini::Ini;

const CREDITS: &str = r#"
Bitcoin Solo Miner - Rust Edition
Credits: x.com/hey_itsmyturn | t.me/itsthealephyouknowfromtwitter
"#;

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
}

impl MiningConfig {
    fn new(address: String, quiet_mode: bool) -> Self {
        Self {
            address,
            current_height: 0,
            quiet_mode,
        }
    }
}

fn load_config() -> Result<(String, bool)> {
    let mut config = Ini::new();
    
    if let Ok(_) = config.load("config.ini") {
        let address = config.get("miner", "wallet_address")
            .unwrap_or_else(|| "".to_string());
        let quiet_mode = match config.getuint("miner", "quiet_mode") {
            Ok(Some(value)) => value == 1,
            _ => false
        };
        
        if !address.is_empty() {
            return Ok((address, quiet_mode));
        }
    }
    
    Ok(("".to_string(), false))
}

async fn get_current_block_height() -> Result<u64> {
    let response = reqwest::get("https://blockchain.info/latestblock").await?;
    let data: Value = response.json().await?;
    Ok(data["height"].as_u64().unwrap_or(0))
}

fn double_sha256(data: &[u8]) -> String {
    let first_hash = Sha256::digest(data);
    let second_hash = Sha256::digest(&first_hash);
    hex::encode(second_hash)
}

fn reverse_hex(hex_str: &str) -> String {
    let mut reversed = String::new();
    for i in (0..hex_str.len()).step_by(2).rev() {
        if i + 1 < hex_str.len() {
            reversed.push_str(&hex_str[i..i+2]);
        }
    }
    reversed
}

fn create_block_header(
    version: &str,
    prevhash: &str,
    merkle_root: &str,
    nbits: &str,
    ntime: &str,
    nonce: &str,
) -> String {
    format!(
        "{}{}{}{}{}{}000000800000000000000000000000000000000000000000000000000000000000000000000000000000000080020000",
        version, prevhash, merkle_root, nbits, ntime, nonce
    )
}

fn calculate_target(nbits: &str) -> String {
    let size = u8::from_str_radix(&nbits[..2], 16).unwrap_or(0);
    let value = &nbits[2..];
    let mut target = value.to_string();
    target.push_str(&"00".repeat((size as usize).saturating_sub(3)));
    target = target.chars().take(64).collect();
    format!("{:0<64}", target)
}

async fn bitcoin_miner(config: Arc<Mutex<MiningConfig>>, restart: bool) -> Result<()> {
    let config_guard = config.lock().unwrap();
    let quiet_mode = config_guard.quiet_mode;
    drop(config_guard);

    if restart {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if !quiet_mode {
            info!("Mining operation restarted");
        }
    } else {
        if !quiet_mode {
            info!("Mining operation initiated");
        }
    }

    if !quiet_mode {
        println!("[*] Connecting to solo.ckpool.org:3333...");
    }
    
    let mut stream = TcpStream::connect("solo.ckpool.org:3333").await?;
    if !quiet_mode {
        println!("[*] Connected to mining pool");
    }
    
    let subscribe_msg = json!({
        "id": 1,
        "method": "mining.subscribe",
        "params": []
    });
    stream.write_all(format!("{}\n", subscribe_msg).as_bytes()).await?;
    if !quiet_mode {
        println!("[*] Subscribing to mining notifications...");
    }

    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    let response = String::from_utf8_lossy(&buffer[..n]);
    
    let lines: Vec<&str> = response.split('\n').collect();
    let response_data: Value = serde_json::from_str(lines[0])?;
    
    let result = &response_data["result"];
    let extranonce1 = &result[1];
    let _extranonce2_size = result[2].as_u64().unwrap_or(0);

    if !quiet_mode {
        println!("[*] Authentication successful");
    }

    let authorize_msg = json!({
        "params": [config.lock().unwrap().address, "password"],
        "id": 2,
        "method": "mining.authorize"
    });
    stream.write_all(format!("{}\n", authorize_msg).as_bytes()).await?;

    if !quiet_mode {
        println!("[*] Waiting for mining job...");
    }
    let mut response_data = String::new();
    while !response_data.contains("mining.notify") {
        let n = stream.read(&mut buffer).await?;
        response_data.push_str(&String::from_utf8_lossy(&buffer[..n]));
    }

    let lines: Vec<&str> = response_data.split('\n').collect();
    let job_line = lines.iter().find(|line| line.contains("mining.notify")).unwrap();
    let job_data: Value = serde_json::from_str(job_line)?;
    let params = &job_data["params"];

    let mining_job = MiningJob {
        job_id: params[0].as_str().unwrap_or("").to_string(),
        prevhash: params[1].as_str().unwrap_or("").to_string(),
        coinb1: params[2].as_str().unwrap_or("").to_string(),
        coinb2: params[3].as_str().unwrap_or("").to_string(),
        merkle_branch: params[4].as_array().unwrap_or(&vec![]).iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect(),
        version: params[5].as_str().unwrap_or("").to_string(),
        nbits: params[6].as_str().unwrap_or("").to_string(),
        ntime: params[7].as_str().unwrap_or("").to_string(),
        clean_jobs: params[8].as_bool().unwrap_or(false),
    };

    let target = calculate_target(&mining_job.nbits);
    let mut rng = rand::thread_rng();
    let extranonce2 = format!("{:0>2}", hex::encode(rng.gen::<[u8; 4]>()));

    let coinbase = format!("{}{}{}{}", 
        mining_job.coinb1, extranonce1, extranonce2, mining_job.coinb2);
    
    let coinbase_hash = double_sha256(&hex::decode(&coinbase).unwrap_or_default());
    let coinbase_hash_bin = hex::decode(&coinbase_hash).unwrap_or_default();

    let mut merkle_root = coinbase_hash_bin.clone();
    for branch in &mining_job.merkle_branch {
        let branch_bytes = hex::decode(branch).unwrap_or_default();
        let mut combined = merkle_root.clone();
        combined.extend_from_slice(&branch_bytes);
        merkle_root = hex::decode(&double_sha256(&combined)).unwrap_or_default();
    }

    let merkle_root_hex = reverse_hex(&hex::encode(&merkle_root));
    
    let work_on = get_current_block_height().await?;
    if !quiet_mode {
        println!("[*] Working on network block height: {}", work_on);
        println!("[*] Current difficulty target: {}", target);
        println!("[*] Starting hash generation...");
    }
    
    let mut hash_count = 0;
    let mut last_log_time = std::time::Instant::now();
    
    loop {
        let current_height = config.lock().unwrap().current_height;
        if current_height > work_on {
            if !quiet_mode {
                println!("[*] New block detected, restarting mining operation");
            }
            break;
        }

        for _ in 0..1000 {
            let nonce = format!("{:0>8}", hex::encode(rng.gen::<[u8; 4]>()));
            let block_header = create_block_header(
                &mining_job.version,
                &mining_job.prevhash,
                &merkle_root_hex,
                &mining_job.nbits,
                &mining_job.ntime,
                &nonce,
            );

            let header_bytes = hex::decode(&block_header).unwrap_or_default();
            let hash = double_sha256(&header_bytes);

            hash_count += 1;

            if hash < target {
                println!("[!] VALID BLOCK HASH DISCOVERED!");
                println!("[*] Hash: {}", hash);
                println!("[*] Target: {}", target);

                let submit_msg = json!({
                    "params": [
                        config.lock().unwrap().address,
                        mining_job.job_id,
                        extranonce2,
                        mining_job.ntime,
                        nonce
                    ],
                    "id": 1,
                    "method": "mining.submit"
                });

                stream.write_all(format!("{}\n", submit_msg).as_bytes()).await?;
                println!("[*] Solution submitted to pool");
                
                let mut response_buffer = [0u8; 1024];
                let n = stream.read(&mut response_buffer).await?;
                let response = String::from_utf8_lossy(&response_buffer[..n]);
                println!("[*] Pool response: {}", response);

                return Ok(());
            }
        }

        if !quiet_mode && last_log_time.elapsed().as_secs() >= 5 {
            let hash_rate = hash_count / 5;
            println!("[*] Hash rate: {} h/s | Total hashes: {}", hash_rate, hash_count);
            hash_count = 0;
            last_log_time = std::time::Instant::now();
        }
    }

    Ok(())
}

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
        
        tokio::time::sleep(Duration::from_secs(40)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("{}", CREDITS);

    let (config_address, config_quiet) = load_config()?;
    
    let address = if !config_address.is_empty() {
        config_address
    } else {
        let mut input = String::new();
        print!("Enter your Bitcoin wallet address for mining rewards: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    let mut quiet_input = String::new();
    print!("Enable quiet mode? (y/n) [default: n]: ");
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut quiet_input)?;
    let quiet_mode = config_quiet || quiet_input.trim().to_lowercase() == "y";

    if !quiet_mode {
        println!("Bitcoin address: {}", address);
        println!("Quiet mode: {}", if quiet_mode { "enabled" } else { "disabled" });
        println!("Starting miner...");
    }

    let config = Arc::new(Mutex::new(MiningConfig::new(address, quiet_mode)));

    let config_clone = Arc::clone(&config);
    let _listener_handle = tokio::spawn(async move {
        if let Err(e) = new_block_listener(config_clone).await {
            error!("Block monitoring error: {}", e);
        }
    });

    loop {
        let config_clone = Arc::clone(&config);
        if let Err(e) = bitcoin_miner(config_clone, false).await {
            error!("Mining operation error: {}", e);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
