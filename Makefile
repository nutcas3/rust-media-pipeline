.PHONY: help build-rust install-python setup run-web run-worker run-all clean test

help:
	@echo "Rust Media Pipeline - Available Commands"
	@echo "========================================"
	@echo "make setup          - Complete setup (build Rust + install Python deps)"
	@echo "make build-rust     - Build the Rust worker binary"
	@echo "make install-python - Install Python dependencies"
	@echo "make run-web        - Run the Flask web server"
	@echo "make run-worker     - Run the RQ worker"
	@echo "make run-all        - Run web server and worker (requires tmux)"
	@echo "make docker-build   - Build Docker images"
	@echo "make docker-up      - Start all services with Docker Compose"
	@echo "make docker-down    - Stop all Docker services"
	@echo "make clean          - Clean build artifacts"
	@echo "make test           - Run tests"

setup: build-rust install-python
	@echo "✅ Setup complete!"

build-rust:
	@echo "🦀 Building Rust worker..."
	cd rust_worker && cargo build --release
	@echo "✅ Rust worker built successfully"

install-python:
	@echo "🐍 Installing Python dependencies..."
	pip install -r python_frontend/requirements.txt
	@echo "✅ Python dependencies installed"

run-web:
	@echo "Starting Flask web server..."
	cd python_frontend && python app.py

run-worker:
	@echo "Starting RQ worker..."
	cd python_frontend && python worker.py

run-all:
	@echo "Starting all services..."
	@echo "Note: This requires tmux to be installed"
	tmux new-session -d -s media_pipeline 'cd python_frontend && python app.py'
	tmux split-window -h -t media_pipeline 'cd python_frontend && python worker.py'
	tmux attach -t media_pipeline

docker-build:
	@echo "🐳 Building Docker images..."
	docker-compose build

docker-up:
	@echo "🐳 Starting Docker services..."
	docker-compose up -d
	@echo "✅ Services started. Web UI: http://localhost:5000"

docker-down:
	@echo "🐳 Stopping Docker services..."
	docker-compose down

clean:
	@echo "🧹 Cleaning build artifacts..."
	cd rust_worker && cargo clean
	find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete
	rm -rf python_frontend/*.egg-info
	@echo "✅ Clean complete"

test:
	@echo "🧪 Running tests..."
	cd rust_worker && cargo test
	@echo "✅ Tests complete"
