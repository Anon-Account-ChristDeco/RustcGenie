"""Test CLI usage of client.py"""

import subprocess
import sys


def test_cli_text_prompt():
    """Test: python client.py -t 'prompt'"""
    result = subprocess.run(
        [sys.executable, "client.py", "-t", "Say hello in one word"],
        capture_output=True,
        text=True,
    )

    print("=== CLI Text Prompt Test ===")
    print(f"Return code: {result.returncode}")
    print(f"Stdout:\n{result.stdout}")
    if result.stderr:
        print(f"Stderr:\n{result.stderr}")

    assert result.returncode == 0, f"CLI failed with: {result.stderr}"
    assert result.stdout.strip(), "No output received"
    print("PASSED\n")


def test_cli_with_all_flag():
    """Test: python client.py -t 'prompt' --all"""
    result = subprocess.run(
        [sys.executable, "client.py", "-t", "Say hi", "--all"],
        capture_output=True,
        text=True,
    )

    print("=== CLI --all Flag Test ===")
    print(f"Return code: {result.returncode}")
    print(f"Stdout (truncated):\n{result.stdout[:500]}...")

    assert result.returncode == 0, f"CLI failed with: {result.stderr}"
    assert "ChatCompletion" in result.stdout or "choices" in result.stdout, "Full response not shown"
    print("PASSED\n")


if __name__ == "__main__":
    test_cli_text_prompt()
    test_cli_with_all_flag()
    print("All CLI tests passed!")
