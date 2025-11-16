import os
import json
import uuid
from pathlib import Path
from flask import Flask, request, jsonify, render_template
from redis import Redis
from rq import Queue
import toml

app = Flask(__name__)

config_path = Path(__file__).parent.parent / "config" / "settings.toml"
config = toml.load(config_path)

redis_conn = Redis.from_url(config["redis"]["url"])
queue = Queue(config["redis"]["queue_name"], connection=redis_conn)

INPUT_PATH = Path(config["storage"]["input_path"])
OUTPUT_PATH = Path(config["storage"]["output_path"])

INPUT_PATH.mkdir(parents=True, exist_ok=True)
OUTPUT_PATH.mkdir(parents=True, exist_ok=True)


@app.route("/")
def index():
    return render_template("index.html")


@app.route("/api/jobs", methods=["GET"])
def list_jobs():
    jobs = []
    
    for job in queue.jobs:
        jobs.append({
            "id": job.id,
            "status": job.get_status(),
            "created_at": job.created_at.isoformat() if job.created_at else None,
            "enqueued_at": job.enqueued_at.isoformat() if job.enqueued_at else None,
        })
    
    return jsonify({"jobs": jobs})


@app.route("/api/jobs/<job_id>", methods=["GET"])
def get_job(job_id):
    from rq.job import Job
    
    try:
        job = Job.fetch(job_id, connection=redis_conn)
        
        result = {
            "id": job.id,
            "status": job.get_status(),
            "created_at": job.created_at.isoformat() if job.created_at else None,
            "enqueued_at": job.enqueued_at.isoformat() if job.enqueued_at else None,
            "started_at": job.started_at.isoformat() if job.started_at else None,
            "ended_at": job.ended_at.isoformat() if job.ended_at else None,
            "result": job.result,
            "exc_info": job.exc_info,
        }
        
        return jsonify(result)
    except Exception as e:
        return jsonify({"error": str(e)}), 404


@app.route("/api/upload", methods=["POST"])
def upload_file():
    if "file" not in request.files:
        return jsonify({"error": "No file provided"}), 400
    
    file = request.files["file"]
    if file.filename == "":
        return jsonify({"error": "Empty filename"}), 400
    
    file_id = str(uuid.uuid4())
    file_ext = Path(file.filename).suffix
    input_filename = f"{file_id}{file_ext}"
    input_path = INPUT_PATH / input_filename
    
    # Save file
    file.save(input_path)
    
    return jsonify({
        "file_id": file_id,
        "filename": input_filename,
        "path": str(input_path)
    })


@app.route("/api/enqueue", methods=["POST"])
def enqueue_job():
    data = request.get_json()
    
    if not data:
        return jsonify({"error": "No JSON data provided"}), 400
    
    required_fields = ["task", "input_path", "output_path"]
    for field in required_fields:
        if field not in data:
            return jsonify({"error": f"Missing required field: {field}"}), 400
    
    job_payload = {
        "task": data["task"],
        "input_path": data["input_path"],
        "output_path": data["output_path"],
        "params": data.get("params", {})
    }
    
    from worker import execute_rust_worker
    
    job = queue.enqueue(
        execute_rust_worker,
        job_payload,
        job_timeout=config["processing"]["timeout_seconds"]
    )
    
    return jsonify({
        "job_id": job.id,
        "status": job.get_status(),
        "payload": job_payload
    })


@app.route("/api/process", methods=["POST"])
def process_file():
    if "file" not in request.files:
        return jsonify({"error": "No file provided"}), 400
    
    file = request.files["file"]
    task = request.form.get("task")
    params = request.form.get("params", "{}")
    
    if not task:
        return jsonify({"error": "No task specified"}), 400
    
    file_id = str(uuid.uuid4())
    file_ext = Path(file.filename).suffix
    input_filename = f"{file_id}{file_ext}"
    output_filename = f"{file_id}_output{file_ext}"
    
    input_path = INPUT_PATH / input_filename
    output_path = OUTPUT_PATH / output_filename
    
    file.save(input_path)
    
    try:
        params_dict = json.loads(params)
    except json.JSONDecodeError:
        params_dict = {}
    
    job_payload = {
        "task": task,
        "input_path": str(input_path),
        "output_path": str(output_path),
        "params": params_dict
    }
    
    from worker import execute_rust_worker
    
    job = queue.enqueue(
        execute_rust_worker,
        job_payload,
        job_timeout=config["processing"]["timeout_seconds"]
    )
    
    return jsonify({
        "job_id": job.id,
        "status": job.get_status(),
        "input_path": str(input_path),
        "output_path": str(output_path)
    })


@app.route("/api/tasks", methods=["GET"])
def list_tasks():
        tasks = {
        "acquisition": [
            {"name": "download_file", "description": "Download file from URL"},
            {"name": "validate_checksum", "description": "Validate file checksum"},
            {"name": "probe_media_file", "description": "Probe media file details"},
            {"name": "split_file_chunks", "description": "Split file into chunks"},
            {"name": "merge_file_chunks", "description": "Merge file chunks"},
            {"name": "sanitize_filename", "description": "Sanitize filename"},
            {"name": "create_file_manifest", "description": "Create file manifest"},
            {"name": "verify_file_integrity", "description": "Verify file integrity"},
        ],
        "video": [
            {"name": "transcode_h264_to_h265", "description": "Convert H.264 to H.265"},
            {"name": "resize_to_720p", "description": "Resize video to 720p"},
            {"name": "get_video_info", "description": "Get video information"},
            {"name": "extract_frames", "description": "Extract frames as images"},
            {"name": "extract_thumbnails", "description": "Extract thumbnail images"},
            {"name": "create_animated_gif", "description": "Create animated GIF"},
            {"name": "detect_scene_cuts", "description": "Detect scene changes"},
            {"name": "apply_watermark", "description": "Apply watermark overlay"},
            {"name": "extract_key_frame", "description": "Extract single frame"},
        ],
        "audio": [
            {"name": "resample_audio", "description": "Resample to different rate"},
            {"name": "extract_audio_from_video", "description": "Extract audio from video"},
            {"name": "get_audio_info", "description": "Get audio information"},
            {"name": "generate_waveform_json", "description": "Generate waveform data"},
            {"name": "mix_audio_tracks", "description": "Mix audio tracks"},
        ],
        "binary": [
            {"name": "calculate_sha256", "description": "Calculate SHA-256 hash"},
            {"name": "compress_archive", "description": "Compress file"},
            {"name": "extract_exif_metadata", "description": "Extract EXIF metadata"},
            {"name": "purge_original_file", "description": "Delete original file"},
            {"name": "validate_format_compliance", "description": "Validate file format"},
            {"name": "chain_job_trigger", "description": "Trigger next job"},
            {"name": "report_metrics", "description": "Report job metrics"},
        ]
    }
    
    return jsonify(tasks)


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, debug=True)
