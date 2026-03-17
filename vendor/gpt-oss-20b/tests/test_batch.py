"""Test batch processing usage"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

TEST_INPUT_DIR = "tests/test_input"
TEST_OUTPUT_DIR = "tests/test_output"


def setup_test_files():
    """Create test input files"""
    os.makedirs(TEST_INPUT_DIR, exist_ok=True)

    # Create sample files
    files = {
        "sample1.txt": "What is Python? Answer briefly.",
        "sample2.txt": "What is Rust? Answer briefly.",
        "subdir/sample3.txt": "What is Go? Answer briefly.",
        "empty.txt": "",  # Empty file to test skip logic
    }

    for filepath, content in files.items():
        full_path = Path(TEST_INPUT_DIR) / filepath
        full_path.parent.mkdir(parents=True, exist_ok=True)
        full_path.write_text(content)

    print(f"Created {len(files)} test files in {TEST_INPUT_DIR}/")


def cleanup():
    """Remove test directories"""
    if os.path.exists(TEST_INPUT_DIR):
        shutil.rmtree(TEST_INPUT_DIR)
    if os.path.exists(TEST_OUTPUT_DIR):
        shutil.rmtree(TEST_OUTPUT_DIR)


def test_batch_processing():
    """Test: python batch_processor.py <input> <output> <pattern> <workers>"""
    print("=== Batch Processing Test ===")

    setup_test_files()

    result = subprocess.run(
        [sys.executable, "batch_processor.py", TEST_INPUT_DIR, TEST_OUTPUT_DIR, "**/*.txt", "1"],
        capture_output=True,
        text=True,
    )

    print(f"Return code: {result.returncode}")
    print(f"Stdout:\n{result.stdout}")
    if result.stderr:
        print(f"Stderr:\n{result.stderr}")

    # Check output structure
    output_path = Path(TEST_OUTPUT_DIR)
    assert output_path.exists(), "Output directory not created"
    assert (output_path / "files_found.txt").exists(), "files_found.txt not created"
    assert (output_path / "raw_responses").exists(), "raw_responses dir not created"
    assert (output_path / "generated_texts").exists(), "generated_texts dir not created"

    # Check that files were processed
    raw_files = list((output_path / "raw_responses").rglob("*.txt"))
    gen_files = list((output_path / "generated_texts").rglob("*.json"))

    print(f"Raw response files: {len(raw_files)}")
    print(f"Generated text files: {len(gen_files)}")

    # Should have processed non-empty files
    assert len(raw_files) >= 1, "No raw response files generated"

    # Print a sample output
    if gen_files:
        sample = gen_files[0].read_text()
        print(f"Sample output ({gen_files[0].name}):\n{sample[:200]}...")

    print("PASSED\n")


def test_batch_output_structure():
    """Verify the output directory structure"""
    print("=== Batch Output Structure Test ===")

    output_path = Path(TEST_OUTPUT_DIR)

    # Check files_found.txt content
    files_found = (output_path / "files_found.txt").read_text()
    print(f"Files found:\n{files_found}")

    # Check processing log
    log_files = list(output_path.glob("processing_log.*.txt"))
    if log_files:
        log_content = log_files[0].read_text()
        print(f"Processing log:\n{log_content}")

        # Verify empty file was skipped
        assert "[EMPTY]" in log_content, "Empty file not marked as EMPTY"

    print("PASSED\n")


if __name__ == "__main__":
    try:
        test_batch_processing()
        test_batch_output_structure()
        print("All batch processing tests passed!")
    finally:
        # Cleanup after tests
        cleanup()
        print("Cleaned up test directories.")
