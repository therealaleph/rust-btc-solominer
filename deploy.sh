#!/bin/bash

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Error handling
error_exit() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check dependencies
check_dependencies() {
    local missing_deps=()
    
    for cmd in ssh scp; do
        if ! command -v "$cmd" &> /dev/null; then
            missing_deps+=("$cmd")
        fi
    done
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        error_exit "Missing required commands: ${missing_deps[*]}. Please install OpenSSH client."
    fi
}

# Global variables for SSH commands
SSH_CMD=""
SSH_PASSWORD=""

# Test SSH connection
test_connection() {
    local server_ip=$1
    local ssh_user=$2
    local use_password=$3
    local ssh_password=$4
    
    info "Testing SSH connection to $ssh_user@$server_ip..."
    
    if [ "$use_password" = "yes" ]; then
        if ! command -v sshpass &> /dev/null; then
            error_exit "sshpass is required for password authentication. Install it with: apt-get install sshpass (Ubuntu/Debian) or brew install hudochenkov/sshpass/sshpass (macOS)"
        fi
        
        if ! sshpass -p "$ssh_password" ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 -o BatchMode=yes "$ssh_user@$server_ip" "echo 'Connection successful'" 2>/dev/null; then
            error_exit "Failed to connect to server. Please check credentials and network connectivity."
        fi
        SSH_PASSWORD="$ssh_password"
        SSH_CMD="sshpass -p '$ssh_password' ssh -o StrictHostKeyChecking=no"
    else
        if ! ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 -o BatchMode=yes "$ssh_user@$server_ip" "echo 'Connection successful'" 2>/dev/null; then
            error_exit "Failed to connect to server. Please check SSH key configuration."
        fi
        SSH_PASSWORD=""
        SSH_CMD="ssh -o StrictHostKeyChecking=no"
    fi
    
    success "SSH connection successful"
}

# Install dependencies on remote server
install_dependencies() {
    local server_ip=$1
    local ssh_user=$2
    
    info "Installing dependencies on remote server (this may take a few minutes)..."
    
    if [ -n "$SSH_PASSWORD" ]; then
        sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" << 'ENDSSH'
set -e
export DEBIAN_FRONTEND=noninteractive

# Update package list
apt-get update -qq

# Install Docker and dependencies
apt-get install -y docker.io docker-compose git ca-certificates curl > /dev/null 2>&1

# Start and enable Docker
systemctl enable docker > /dev/null 2>&1
systemctl start docker > /dev/null 2>&1

# Wait for Docker to be ready
sleep 3

# Verify Docker installation
if ! docker --version > /dev/null 2>&1; then
    echo "ERROR: Docker installation failed"
    exit 1
fi

echo "SUCCESS: Dependencies installed successfully"
ENDSSH
    else
        ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" << 'ENDSSH'
set -e
export DEBIAN_FRONTEND=noninteractive

# Update package list
apt-get update -qq

# Install Docker and dependencies
apt-get install -y docker.io docker-compose git ca-certificates curl > /dev/null 2>&1

# Start and enable Docker
systemctl enable docker > /dev/null 2>&1
systemctl start docker > /dev/null 2>&1

# Wait for Docker to be ready
sleep 3

# Verify Docker installation
if ! docker --version > /dev/null 2>&1; then
    echo "ERROR: Docker installation failed"
    exit 1
fi

echo "SUCCESS: Dependencies installed successfully"
ENDSSH
    fi

    if [ $? -ne 0 ]; then
        error_exit "Failed to install dependencies on remote server"
    fi
    
    success "Dependencies installed successfully"
}

# Clone repository on remote server
setup_repository() {
    local server_ip=$1
    local ssh_user=$2
    
    info "Setting up repository on remote server..."
    
    if [ -n "$SSH_PASSWORD" ]; then
        sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" << 'ENDSSH'
set -e

APP_DIR="/root/bitcoin-miner"
mkdir -p "$APP_DIR"
cd "$APP_DIR"

# Clone or update repository
if [ -d ".git" ]; then
    git pull origin main > /dev/null 2>&1 || git pull > /dev/null 2>&1
else
    git clone https://github.com/therealaleph/rust-btc-solominer.git . > /dev/null 2>&1
fi

# Create logs directory
mkdir -p logs

echo "SUCCESS: Repository setup completed"
ENDSSH
    else
        ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" << 'ENDSSH'
set -e

APP_DIR="/root/bitcoin-miner"
mkdir -p "$APP_DIR"
cd "$APP_DIR"

# Clone or update repository
if [ -d ".git" ]; then
    git pull origin main > /dev/null 2>&1 || git pull > /dev/null 2>&1
else
    git clone https://github.com/therealaleph/rust-btc-solominer.git . > /dev/null 2>&1
fi

# Create logs directory
mkdir -p logs

echo "SUCCESS: Repository setup completed"
ENDSSH
    fi

    if [ $? -ne 0 ]; then
        error_exit "Failed to setup repository on remote server"
    fi
    
    success "Repository setup completed"
}

# Create docker-compose.yml with user credentials
create_docker_compose() {
    local server_ip=$1
    local ssh_user=$2
    local btc_address=$3
    local tg_token=$4
    local tg_user_id=$5
    
    info "Creating docker-compose.yml configuration..."
    
    # Escape variables for heredoc
    local escaped_btc_address=$(printf '%s\n' "$btc_address" | sed "s/'/'\\\\''/g")
    local escaped_tg_token=$(printf '%s\n' "$tg_token" | sed "s/'/'\\\\''/g")
    local escaped_tg_user_id=$(printf '%s\n' "$tg_user_id" | sed "s/'/'\\\\''/g")
    
    local compose_content
    compose_content="services:
  bitcoin-miner:
    build: .
    container_name: bitcoin-solo-miner
    restart: always
    environment:
      - BTC_ADDRESS=${escaped_btc_address}
      - QUIET_MODE=0
      - RUST_LOG=info
      - DOCKER_CONTAINER=1"
    
    # Add Telegram credentials only if both are provided
    if [ -n "$tg_token" ] && [ -n "$tg_user_id" ]; then
        compose_content="${compose_content}
      - TELEGRAM_BOT_TOKEN=${escaped_tg_token}
      - TELEGRAM_USER_ID=${escaped_tg_user_id}"
    fi
    
    compose_content="${compose_content}
    volumes:
      - ./logs:/app/logs
    stdin_open: false
    tty: false
    logging:
      driver: \"json-file\"
      options:
        max-size: \"10m\"
        max-file: \"3\"
        labels: \"bitcoin-miner\""
    
    if [ -n "$SSH_PASSWORD" ]; then
        sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "cat > /root/bitcoin-miner/docker-compose.yml" <<< "$compose_content"
    else
        ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "cat > /root/bitcoin-miner/docker-compose.yml" <<< "$compose_content"
    fi
    
    if [ $? -ne 0 ]; then
        error_exit "Failed to create configuration file"
    fi
    
    # Verify file was created
    if [ -n "$SSH_PASSWORD" ]; then
        if ! sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "test -f /root/bitcoin-miner/docker-compose.yml"; then
            error_exit "Configuration file was not created"
        fi
    else
        if ! ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "test -f /root/bitcoin-miner/docker-compose.yml"; then
            error_exit "Configuration file was not created"
        fi
    fi
    
    success "Configuration file created"
}

# Build and start Docker container
deploy_container() {
    local server_ip=$1
    local ssh_user=$2
    
    info "Building and starting Docker container (this may take several minutes)..."
    
    if [ -n "$SSH_PASSWORD" ]; then
        sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" << 'ENDSSH'
set -e

cd /root/bitcoin-miner

# Stop existing container if running
docker-compose down 2>/dev/null || true

# Build and start container
docker-compose up --build -d

# Wait a moment for container to start
sleep 5

# Check if container is running
if ! docker-compose ps | grep -q "Up"; then
    echo "ERROR: Container failed to start"
    docker-compose logs
    exit 1
fi

echo "SUCCESS: Container deployed and running"
ENDSSH
    else
        ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" << 'ENDSSH'
set -e

cd /root/bitcoin-miner

# Stop existing container if running
docker-compose down 2>/dev/null || true

# Build and start container
docker-compose up --build -d

# Wait a moment for container to start
sleep 5

# Check if container is running
if ! docker-compose ps | grep -q "Up"; then
    echo "ERROR: Container failed to start"
    docker-compose logs
    exit 1
fi

echo "SUCCESS: Container deployed and running"
ENDSSH
    fi

    if [ $? -ne 0 ]; then
        error_exit "Failed to deploy container. Check logs on server for details."
    fi
    
    success "Container deployed and running"
}

# Display status and logs
show_status() {
    local server_ip=$1
    local ssh_user=$2
    
    info "Fetching container status..."
    
    echo ""
    echo "=== Container Status ==="
    if [ -n "$SSH_PASSWORD" ]; then
        sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "cd /root/bitcoin-miner && docker-compose ps" 2>/dev/null || true
    else
        ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "cd /root/bitcoin-miner && docker-compose ps" 2>/dev/null || true
    fi
    
    echo ""
    echo "=== Recent Logs (last 20 lines) ==="
    if [ -n "$SSH_PASSWORD" ]; then
        sshpass -p "$SSH_PASSWORD" ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "cd /root/bitcoin-miner && docker-compose logs --tail=20" 2>/dev/null || true
    else
        ssh -o StrictHostKeyChecking=no "$ssh_user@$server_ip" "cd /root/bitcoin-miner && docker-compose logs --tail=20" 2>/dev/null || true
    fi
}

# Input validation
validate_input() {
    local value=$1
    local name=$2
    
    if [ -z "$value" ]; then
        error_exit "$name cannot be empty"
    fi
}

validate_btc_address() {
    local address=$1
    
    if [ ${#address} -lt 26 ] || [ ${#address} -gt 35 ]; then
        warning "Bitcoin address length is unusual (${#address} characters). Continuing anyway..."
    fi
}

# Detect if sudo is needed for Docker commands and check if daemon is running
detect_docker_sudo() {
    local docker_prefix=""
    
    # Check if Docker daemon is running (try without sudo first)
    if docker info &> /dev/null 2>&1; then
        docker_prefix=""
    elif sudo docker info &> /dev/null 2>&1; then
        docker_prefix="sudo "
        info "Docker requires sudo privileges. Using sudo for Docker commands."
    else
        # Check if Docker is installed
        if ! command -v docker &> /dev/null; then
            error_exit "Docker is not installed. Please install Docker first."
        fi
        
        # Docker is installed but daemon is not running
        error_exit "Docker daemon is not running. Please start Docker Desktop or the Docker service:\n  - macOS: Start Docker Desktop application\n  - Linux: Run 'sudo systemctl start docker' or 'sudo service docker start'"
    fi
    
    echo "$docker_prefix"
}

# Local deployment function
deploy_local() {
    echo "=========================================="
    echo "Bitcoin Solo Miner - Local Deployment"
    echo "=========================================="
    echo ""
    
    # Check if Docker is installed
    if ! command -v docker &> /dev/null; then
        error_exit "Docker is not installed. Please install Docker first."
    fi
    
    # Detect if sudo is needed for Docker
    DOCKER_SUDO=$(detect_docker_sudo)
    
    # Check for Docker Compose
    local compose_available=false
    if command -v docker-compose &> /dev/null; then
        DOCKER_COMPOSE_CMD="${DOCKER_SUDO}docker-compose"
        compose_available=true
    elif ${DOCKER_SUDO}docker compose version &> /dev/null 2>&1; then
        DOCKER_COMPOSE_CMD="${DOCKER_SUDO}docker compose"
        compose_available=true
    fi
    
    if [ "$compose_available" = false ]; then
        error_exit "Docker Compose is not installed. Please install Docker Compose first."
    fi
    
    # Setup repository - check if we're in the repository directory
    local original_dir=$(pwd)
    if [ -f "Dockerfile" ] && [ -f "Cargo.toml" ] && [ -f "src/main.rs" ]; then
        info "Using existing repository in current directory"
    else
        # Clone repository to a temporary directory
        info "Repository not found in current directory. Cloning repository..."
        
        local temp_dir=$(mktemp -d)
        local repo_dir="$temp_dir/rust-btc-solominer"
        
        if ! git clone https://github.com/therealaleph/rust-btc-solominer.git "$repo_dir" 2>/dev/null; then
            rm -rf "$temp_dir"
            error_exit "Failed to clone repository. Please check your internet connection."
        fi
        
        success "Repository cloned successfully"
        echo ""
        echo "Repository cloned to: $repo_dir"
        echo "You can delete this directory after deployment if desired."
        echo ""
        
        # Change to repository directory
        cd "$repo_dir" || error_exit "Failed to change to repository directory"
    fi
    
    # Store current directory for later use
    local deploy_dir=$(pwd)
    
    # Get user credentials
    echo "Enter your configuration details:"
    
    read -p "Bitcoin address (required): " BTC_ADDRESS
    validate_input "$BTC_ADDRESS" "Bitcoin address"
    validate_btc_address "$BTC_ADDRESS"
    
    read -p "Telegram bot token (optional, press Enter to skip): " TG_TOKEN
    TG_USER_ID=""
    
    # Only ask for user ID if token was provided
    if [ -n "$TG_TOKEN" ]; then
        read -p "Telegram user ID (required if token provided): " TG_USER_ID
        
        if [ -n "$TG_USER_ID" ]; then
            # Validate Telegram user ID is numeric
            if ! [[ "$TG_USER_ID" =~ ^[0-9]+$ ]]; then
                error_exit "Telegram user ID must be numeric"
            fi
        else
            warning "Telegram token provided but user ID is empty. Telegram notifications will be disabled."
            TG_TOKEN=""
        fi
    fi
    
    # Create docker-compose.yml
    info "Creating docker-compose.yml configuration..."
    
    local compose_content
    compose_content="services:
  bitcoin-miner:
    build: .
    container_name: bitcoin-solo-miner
    restart: always
    environment:
      - BTC_ADDRESS=$BTC_ADDRESS
      - QUIET_MODE=0
      - RUST_LOG=info
      - DOCKER_CONTAINER=1"
    
    # Add Telegram credentials only if both are provided
    if [ -n "$TG_TOKEN" ] && [ -n "$TG_USER_ID" ]; then
        compose_content="${compose_content}
      - TELEGRAM_BOT_TOKEN=$TG_TOKEN
      - TELEGRAM_USER_ID=$TG_USER_ID"
    fi
    
    compose_content="${compose_content}
    volumes:
      - ./logs:/app/logs
    stdin_open: false
    tty: false
    logging:
      driver: \"json-file\"
      options:
        max-size: \"10m\"
        max-file: \"3\"
        labels: \"bitcoin-miner\""
    
    echo "$compose_content" > docker-compose.yml
    success "Configuration file created"
    
    # Create logs directory
    mkdir -p logs
    
    # Build and start container
    info "Building and starting Docker container (this may take several minutes)..."
    
    $DOCKER_COMPOSE_CMD down 2>/dev/null || true
    $DOCKER_COMPOSE_CMD up --build -d
    
    if [ $? -ne 0 ]; then
        error_exit "Failed to deploy container. Check logs for details."
    fi
    
    # Wait a moment for container to start
    sleep 5
    
    # Check if container is running
    if ! $DOCKER_COMPOSE_CMD ps 2>/dev/null | grep -q "Up"; then
        error_exit "Container failed to start"
    fi
    
    success "Container deployed and running"
    
    echo ""
    echo "=== Container Status ==="
    $DOCKER_COMPOSE_CMD ps
    
    echo ""
    echo "=== Recent Logs (last 20 lines) ==="
    $DOCKER_COMPOSE_CMD logs --tail=20
    
    echo ""
    echo "=========================================="
    success "Local deployment completed successfully!"
    echo "=========================================="
    echo ""
    echo "Your Bitcoin solo miner is now running locally"
    echo ""
    if [ "$deploy_dir" != "$original_dir" ]; then
        echo "Repository location: $deploy_dir"
        echo ""
    fi
    echo "To view logs: cd $deploy_dir && $DOCKER_COMPOSE_CMD logs -f"
    echo "To stop miner: cd $deploy_dir && $DOCKER_COMPOSE_CMD down"
    echo "To restart: cd $deploy_dir && $DOCKER_COMPOSE_CMD restart"
    echo ""
}

# Remote deployment function
deploy_remote() {
    echo "=========================================="
    echo "Bitcoin Solo Miner - Remote Deployment"
    echo "=========================================="
    echo ""
    
    # Check dependencies
    check_dependencies
    
    # Get server connection details
    read -p "Enter server IP address: " SERVER_IP
    validate_input "$SERVER_IP" "Server IP address"
    
    read -p "Enter SSH username [default: root]: " SSH_USER
    SSH_USER=${SSH_USER:-root}
    validate_input "$SSH_USER" "SSH username"
    
    read -p "Do you want to use SSH key authentication? (yes/no) [default: yes]: " USE_SSH_KEY
    USE_SSH_KEY=${USE_SSH_KEY:-yes}
    
    SSH_PASSWORD=""
    if [ "$USE_SSH_KEY" != "yes" ]; then
        read -sp "Enter SSH password: " SSH_PASSWORD
        echo ""
        validate_input "$SSH_PASSWORD" "SSH password"
    fi
    
    # Test connection
    test_connection "$SERVER_IP" "$SSH_USER" "$USE_SSH_KEY" "$SSH_PASSWORD"
    
    # Get user credentials
    echo ""
    echo "Enter your configuration details:"
    
    read -p "Bitcoin address (required): " BTC_ADDRESS
    validate_input "$BTC_ADDRESS" "Bitcoin address"
    validate_btc_address "$BTC_ADDRESS"
    
    read -p "Telegram bot token (optional, press Enter to skip): " TG_TOKEN
    TG_USER_ID=""
    
    # Only ask for user ID if token was provided
    if [ -n "$TG_TOKEN" ]; then
        read -p "Telegram user ID (required if token provided): " TG_USER_ID
        
        if [ -n "$TG_USER_ID" ]; then
            # Validate Telegram user ID is numeric
            if ! [[ "$TG_USER_ID" =~ ^[0-9]+$ ]]; then
                error_exit "Telegram user ID must be numeric"
            fi
        else
            warning "Telegram token provided but user ID is empty. Telegram notifications will be disabled."
            TG_TOKEN=""
        fi
    fi
    
    echo ""
    info "Starting deployment process..."
    echo ""
    
    # Install dependencies
    install_dependencies "$SERVER_IP" "$SSH_USER"
    
    # Setup repository
    setup_repository "$SERVER_IP" "$SSH_USER"
    
    # Create docker-compose.yml
    create_docker_compose "$SERVER_IP" "$SSH_USER" "$BTC_ADDRESS" "$TG_TOKEN" "$TG_USER_ID"
    
    # Deploy container
    deploy_container "$SERVER_IP" "$SSH_USER"
    
    # Show status
    echo ""
    show_status "$SERVER_IP" "$SSH_USER"
    
    echo ""
    echo "=========================================="
    success "Deployment completed successfully!"
    echo "=========================================="
    echo ""
    echo "Your Bitcoin solo miner is now running on: $SERVER_IP"
    echo ""
    echo "To view logs: ssh $SSH_USER@$SERVER_IP 'cd /root/bitcoin-miner && docker-compose logs -f'"
    echo "To stop miner: ssh $SSH_USER@$SERVER_IP 'cd /root/bitcoin-miner && docker-compose down'"
    echo "To restart: ssh $SSH_USER@$SERVER_IP 'cd /root/bitcoin-miner && docker-compose restart'"
    echo ""
}

# Main deployment function
main() {
    echo "=========================================="
    echo "Bitcoin Solo Miner - Deployment Script"
    echo "=========================================="
    echo ""
    
    read -p "Deploy to (local/remote) [default: remote]: " DEPLOY_TYPE
    DEPLOY_TYPE=${DEPLOY_TYPE:-remote}
    
    case "$DEPLOY_TYPE" in
        local|Local|LOCAL)
            deploy_local
            ;;
        remote|Remote|REMOTE)
            deploy_remote
            ;;
        *)
            error_exit "Invalid deployment type. Choose 'local' or 'remote'"
            ;;
    esac
}

# Run main function
main "$@"

