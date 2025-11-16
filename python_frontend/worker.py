import json
import subprocess
import sys
from pathlib import Path
import toml

config_path = Path(__file__).parent.parent / "config" / "settings.toml"
config = toml.load(config_path)

RUST_WORKER_PATH = Path(__file__).parent.parent / "rust_worker" / "target" / "release" / "rust_worker"


def execute_rust_worker(job_payload):
    job_payload (dict): Job parameters including task, input_path, output_path, and params
    dict: Result from the Rust worker
    Raises:
        RuntimeError: If the Rust worker fails

    if not RUST_WORKER_PATH.exists():
        raise FileNotFoundError(
            f"Rust worker binary not found at {RUST_WORKER_PATH}. "
            "Please build it first with: cd rust_worker && cargo build --release"
        )
    
    job_json = json.dumps(job_payload)
    
    try:
        result = subprocess.run(
            [str(RUST_WORKER_PATH), job_json],
            capture_output=True,
            text=True,
            timeout=config["processing"]["timeout_seconds"],
            check=False
        )

        if result.returncode == 0:
            try:
                output = json.loads(result.stdout)
                return output
            except json.JSONDecodeError:
                return {
                    "success": True,
                    "message": "Job completed",
                    "output": result.stdout,
                    "stderr": result.stderr
                }
        else:
            try:
                error_output = json.loads(result.stdout)
                raise RuntimeError(f"Rust worker failed: {error_output.get('message', 'Unknown error')}")
            except json.JSONDecodeError:
                raise RuntimeError(
                    f"Rust worker failed with exit code {result.returncode}\n"
                    f"stdout: {result.stdout}\n"
                    f"stderr: {result.stderr}"
                )
    
    except subprocess.TimeoutExpired:
        raise RuntimeError(
            f"Job timed out after {config['processing']['timeout_seconds']} seconds"
        )
    except Exception as e:
        raise RuntimeError(f"Failed to execute Rust worker: {str(e)}")


if __name__ == "__main__":
    from redis import Redis
    from rq import Worker, Queue, Connection
    
    redis_conn = Redis.from_url(config["redis"]["url"])
    
    with Connection(redis_conn):
        worker = Worker([config["redis"]["queue_name"]])
        print(f"Starting RQ worker for queue: {config['redis']['queue_name']}")
        print(f"Redis URL: {config['redis']['url']}")
        print(f"Rust worker binary: {RUST_WORKER_PATH}")
        worker.work()
