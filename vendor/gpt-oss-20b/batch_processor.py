"""Batch processor for running GPT-OSS-20B on multiple files."""

import os
import sys
import time
from functools import partial
from multiprocessing import Pool, Value, Lock
from pathlib import Path

from tqdm import tqdm

from client import GPTOSSClient
from config import Config


DEFAULT_MAX_RPM = 300


class RateLimiter:
    """Token-bucket style rate limiter for multiprocessing.

    Each worker reserves a time slot, releases the lock, then sleeps
    until its slot arrives. This spreads requests evenly across time.
    """

    def __init__(self, max_rpm: int):
        self.max_rpm = max_rpm
        self.interval = 60.0 / max_rpm  # minimum seconds between requests
        self._next_slot = Value('d', 0.0)  # next allowed request time
        self._lock = Lock()

    def acquire(self):
        """Wait until a request slot is available."""
        with self._lock:
            now = time.monotonic()
            wait_time = max(0.0, self._next_slot.value - now)
            self._next_slot.value = max(now, self._next_slot.value) + self.interval

        if wait_time > 0:
            time.sleep(wait_time)


class BatchConfig:
    """Configuration for batch processing."""

    def __init__(self, input_dir: str, output_dir: str, file_pattern: str = "**/*.rs"):
        self.input_dir = Path(input_dir).resolve()
        self.output_dir = Path(output_dir).resolve()
        self.file_pattern = file_pattern
        self.raw_dir = self.output_dir / "raw_responses"
        self.generated_dir = self.output_dir / "generated_texts"

        os.makedirs(self.raw_dir, exist_ok=True)
        os.makedirs(self.generated_dir, exist_ok=True)


# Module-level globals set by init_worker
WORKER_ID = 0
RATE_LIMITER: RateLimiter | None = None


def init_worker(counter, lock, rate_limiter):
    """Initialize worker with unique ID and shared rate limiter."""
    global WORKER_ID, RATE_LIMITER
    with lock:
        counter.value += 1
        WORKER_ID = counter.value
    RATE_LIMITER = rate_limiter


def process_file(batch_config: BatchConfig, input_filepath: Path) -> int:
    """Process a single file through the model."""
    global WORKER_ID, RATE_LIMITER
    idx = WORKER_ID

    client = GPTOSSClient()
    log_path = batch_config.output_dir / f"processing_log.{idx}.txt"

    with open(input_filepath, "r", encoding="utf-8") as f:
        content = f.read()

    if not content.strip():
        with open(log_path, "a", encoding="utf-8") as log:
            log.write(f"[EMPTY] {input_filepath.relative_to(batch_config.input_dir)}\n")
        return idx

    relative_path = input_filepath.relative_to(batch_config.input_dir)
    filename = input_filepath.stem

    raw_path = batch_config.raw_dir / relative_path.parent / f"{filename}.txt"
    gen_path = batch_config.generated_dir / relative_path.parent / f"{filename}.json"

    os.makedirs(raw_path.parent, exist_ok=True)
    os.makedirs(gen_path.parent, exist_ok=True)

    # Wait for rate limiter before making API call
    if RATE_LIMITER is not None:
        RATE_LIMITER.acquire()

    try:
        response = client.query(content)
    except Exception as e:
        with open(log_path, "a", encoding="utf-8") as log:
            log.write(f"[ERROR] {relative_path}: {e}\n")
        return idx

    with open(raw_path, "w", encoding="utf-8") as f:
        f.write(str(response))

    if not response.choices:
        with open(log_path, "a", encoding="utf-8") as log:
            log.write(f"[NO_CHOICES] {relative_path}\n")
        return idx

    generated_text = response.choices[0].message.content
    if generated_text:
        with open(gen_path, "w", encoding="utf-8") as f:
            f.write(generated_text)
        with open(log_path, "a", encoding="utf-8") as log:
            log.write(f"[PROCESSED] {relative_path}\n")
    else:
        with open(log_path, "a", encoding="utf-8") as log:
            log.write(f"[GEN_NONE] {relative_path}\n")

    return idx


def _format_duration(seconds: float) -> str:
    """Format seconds into a human-readable duration string."""
    if seconds < 60:
        return f"{seconds:.0f}s"
    minutes = int(seconds) // 60
    secs = int(seconds) % 60
    if minutes < 60:
        return f"{minutes}m {secs}s"
    hours = minutes // 60
    mins = minutes % 60
    return f"{hours}h {mins}m"


def run_batch(
    input_dir: str,
    output_dir: str,
    file_pattern: str = "**/*.rs",
    num_workers: int = 1,
    max_rpm: int = DEFAULT_MAX_RPM,
):
    """Run batch processing on all matching files.

    Args:
        input_dir: Directory containing input files.
        output_dir: Directory for output files.
        file_pattern: Glob pattern for matching input files.
        num_workers: Number of parallel worker processes.
        max_rpm: Maximum requests per minute (rate limit).
    """
    batch_config = BatchConfig(input_dir, output_dir, file_pattern)

    files = list(batch_config.input_dir.glob(file_pattern))
    total_files = len(files)

    # Pre-run summary
    # min_duration: theoretical minimum time based only on rate limiter spacing.
    # The rate limiter enforces a gap of (60 / max_rpm) seconds between requests.
    # Even if every API call returned instantly (0 latency), this is the minimum
    # time needed. Actual duration will be longer due to API latency.
    # For small file counts, min_duration may be very short (e.g. 3s), meaning
    # the rate limiter is not the bottleneck — API latency dominates instead.
    min_duration = total_files / max_rpm * 60  # seconds
    print(f"{'='*50}")
    print(f"  Batch Processing Summary")
    print(f"{'='*50}")
    print(f"  Files found:    {total_files}")
    print(f"  Workers:        {num_workers}")
    print(f"  Rate limit:     {max_rpm} requests/min")
    print(f"  Min duration:   {_format_duration(min_duration)} (excluding API latency)")
    print(f"{'='*50}")

    if total_files == 0:
        print("No files to process.")
        return

    with open(batch_config.output_dir / "files_found.txt", "w", encoding="utf-8") as f:
        for filepath in files:
            f.write(str(filepath) + "\n")

    counter = Value('i', 0)
    lock = Lock()
    rate_limiter = RateLimiter(max_rpm)

    with Pool(processes=num_workers, initializer=init_worker, initargs=(counter, lock, rate_limiter)) as pool:
        func = partial(process_file, batch_config)
        for _ in tqdm(pool.imap_unordered(func, files), total=total_files):
            pass


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python batch_processor.py <input_dir> <output_dir> [file_pattern] [num_workers] [max_rpm]")
        print("  file_pattern: glob pattern (default: **/*.rs)")
        print("  num_workers:  number of parallel workers (default: 1)")
        print(f"  max_rpm:      max requests per minute (default: {DEFAULT_MAX_RPM})")
        sys.exit(1)

    input_dir = sys.argv[1]
    output_dir = sys.argv[2]
    file_pattern = sys.argv[3] if len(sys.argv) > 3 else "**/*.rs"
    num_workers = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    max_rpm = int(sys.argv[5]) if len(sys.argv) > 5 else DEFAULT_MAX_RPM

    run_batch(input_dir, output_dir, file_pattern, num_workers, max_rpm)
