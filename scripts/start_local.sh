#!/bin/bash

# Start local development environment for Rust Media Pipeline

set -e

echo "Starting Rust Media Pipeline (Local Development)"
echo "=================================================="

# Check if Redis is running
if ! redis-cli ping > /dev/null 2>&1; then
    echo "❌ Redis is not running. Please start Redis first:"
    echo "   brew services start redis  (macOS)"
    echo "   sudo systemctl start redis (Linux)"
    exit 1
fi

echo "✅ Redis is running"

# Check if Rust binary exists
if [ ! -f "rust_worker/target/release/rust_worker" ]; then
    echo "⚠️  Rust worker binary not found. Building..."
    cd rust_worker
    cargo build --release
    cd ..
    echo "✅ Rust worker built"
fi

# Create data directories
mkdir -p data/input data/output

# Start Flask web server in background
echo "🌐 Starting Flask web server on http://localhost:5000"
cd python_frontend
python app.py &
WEB_PID=$!
cd ..

# Wait a moment for web server to start
sleep 2

# Start RQ worker
echo "⚙️  Starting RQ worker"
cd python_frontend
python worker.py &
WORKER_PID=$!
cd ..

echo ""
echo "✅ All services started!"
echo "=================================================="
echo "Web UI:     http://localhost:5000"
echo "Redis:      localhost:6379"
echo ""
echo "Process IDs:"
echo "  Web:      $WEB_PID"
echo "  Worker:   $WORKER_PID"
echo ""
echo "To stop all services, run:"
echo "  kill $WEB_PID $WORKER_PID"
echo ""
echo "Press Ctrl+C to stop all services"

# Wait for interrupt
trap "echo ''; echo '🛑 Stopping services...'; kill $WEB_PID $WORKER_PID 2>/dev/null; exit 0" INT TERM

# Keep script running
wait
