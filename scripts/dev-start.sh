#!/usr/bin/env bash
# ============================================================================
# RusToK Development Stack Startup Script
# ============================================================================
# This script starts all development services:
# - PostgreSQL database
# - RusToK Axum server
# - Next.js admin panel (port 3000)
# - Leptos admin panel (port 3001)
# - Next.js storefront (port 3100)
# - Leptos storefront (port 3101)
#
# Usage:
#   ./scripts/dev-start.sh              # Start all services
#   ./scripts/dev-start.sh --profile admin    # Start only admin services
#   ./scripts/dev-start.sh --stop       # Stop all services
#   ./scripts/dev-start.sh --restart    # Restart all services
#   ./scripts/dev-start.sh --logs       # Follow logs

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

# ============================================================================
# Helper functions
# ============================================================================

print_header() {
    echo -e "${BLUE}============================================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}============================================================================${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# Check if .env.dev exists, if not create from example
check_env() {
    if [ ! -f .env.dev ]; then
        print_warning ".env.dev not found, creating from .env.dev.example..."
        cp .env.dev.example .env.dev
        print_success "Created .env.dev - please review and adjust if needed"
    fi
}

# Start services
start_services() {
    local profile=${1:-full-dev}
    
    print_header "Starting RusToK Development Stack (profile: $profile)"
    
    check_env
    
    print_info "Starting services with Docker Compose..."
    docker compose \
        --env-file .env.dev \
        -f docker-compose.yml \
        -f docker-compose.full-dev.yml \
        --profile "$profile" \
        up -d --build
    
    print_success "Services started!"
    print_service_urls
    wait_for_health
}

# Stop services
stop_services() {
    print_header "Stopping RusToK Development Stack"
    
    docker compose \
        -f docker-compose.yml \
        -f docker-compose.full-dev.yml \
        down
    
    print_success "All services stopped"
}

# Restart services
restart_services() {
    stop_services
    sleep 2
    start_services "$@"
}

# Show logs
show_logs() {
    local service=${1:-}
    
    if [ -z "$service" ]; then
        docker compose \
            -f docker-compose.yml \
            -f docker-compose.full-dev.yml \
            logs -f
    else
        docker compose \
            -f docker-compose.yml \
            -f docker-compose.full-dev.yml \
            logs -f "$service"
    fi
}

# Print service URLs
print_service_urls() {
    echo ""
    print_header "Service URLs"
    echo -e "${GREEN}Backend:${NC}"
    echo -e "  Server API:        ${BLUE}http://localhost:5150${NC}"
    echo -e "  GraphQL Endpoint:  ${BLUE}http://localhost:5150/api/graphql${NC}"
    echo -e "  Health Check:      ${BLUE}http://localhost:5150/api/health${NC}"
    echo ""
    echo -e "${GREEN}Admin Panels:${NC}"
    echo -e "  Next.js Admin:     ${BLUE}http://localhost:3000${NC}"
    echo -e "  Leptos Admin:      ${BLUE}http://localhost:3001${NC}"
    echo ""
    echo -e "${GREEN}Storefronts:${NC}"
    echo -e "  Next.js Storefront: ${BLUE}http://localhost:3100${NC}"
    echo -e "  Leptos Storefront:  ${BLUE}http://localhost:3101${NC}"
    echo ""
    echo -e "${GREEN}Database:${NC}"
    echo -e "  PostgreSQL:        ${BLUE}localhost:5432${NC}"
    echo -e "  Database:          ${BLUE}rustok_dev${NC}"
    echo -e "  User:              ${BLUE}rustok${NC}"
    echo ""
    echo -e "${YELLOW}Default Admin Credentials (dev only):${NC}"
    echo -e "  Email:             ${BLUE}admin@local${NC}"
    echo -e "  Password:          ${BLUE}admin12345${NC}"
    echo ""
}

# Wait for services to be healthy
wait_for_health() {
    print_info "Waiting for services to be healthy..."
    
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -sf http://localhost:5150/api/health > /dev/null 2>&1; then
            print_success "Server is healthy!"
            return 0
        fi
        
        attempt=$((attempt + 1))
        echo -n "."
        sleep 2
    done
    
    print_error "Server health check timeout. Check logs with: ./scripts/dev-start.sh --logs server"
    return 1
}

# Show status
show_status() {
    print_header "RusToK Development Stack Status"
    docker compose \
        -f docker-compose.yml \
        -f docker-compose.full-dev.yml \
        ps
}

# ============================================================================
# Main script
# ============================================================================

case "${1:-start}" in
    start)
        start_services "${2:-full-dev}"
        ;;
    stop)
        stop_services
        ;;
    restart)
        restart_services "${2:-full-dev}"
        ;;
    logs)
        show_logs "${2:-}"
        ;;
    status)
        show_status
        ;;
    --profile)
        start_services "$2"
        ;;
    --stop)
        stop_services
        ;;
    --restart)
        restart_services "${2:-full-dev}"
        ;;
    --logs)
        show_logs "${2:-}"
        ;;
    --status)
        show_status
        ;;
    --help|-h)
        echo "Usage: $0 [command] [options]"
        echo ""
        echo "Commands:"
        echo "  start [profile]    Start all services (default profile: full-dev)"
        echo "  stop               Stop all services"
        echo "  restart [profile]  Restart all services"
        echo "  logs [service]     Follow logs (all services or specific service)"
        echo "  status             Show service status"
        echo ""
        echo "Profiles:"
        echo "  full-dev           All services (default)"
        echo "  admin              Only admin panels + server + db"
        echo "  storefront         Only storefronts + server + db"
        echo "  server             Only server + db"
        echo ""
        echo "Examples:"
        echo "  $0 start                  # Start all services"
        echo "  $0 start admin            # Start only admin services"
        echo "  $0 logs server            # Follow server logs"
        echo "  $0 restart                # Restart all services"
        echo "  $0 stop                   # Stop all services"
        ;;
    *)
        print_error "Unknown command: $1"
        echo "Run '$0 --help' for usage information"
        exit 1
        ;;
esac
