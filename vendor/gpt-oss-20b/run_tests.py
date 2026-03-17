#!/usr/bin/env python
"""Run all usage tests."""

import sys


def main():
    print("=" * 50)
    print("GPT-OSS-20B Usage Tests")
    print("=" * 50)
    print()

    # Import and run each test module
    from tests import test_cli, test_python_api, test_batch

    print("1/3 Running CLI tests...")
    print("-" * 50)
    test_cli.test_cli_text_prompt()
    test_cli.test_cli_with_all_flag()

    print("2/3 Running Python API tests...")
    print("-" * 50)
    test_python_api.test_default_config()
    test_python_api.test_custom_config()
    test_python_api.test_get_text_method()

    print("3/3 Running Batch Processing tests...")
    print("-" * 50)
    try:
        test_batch.test_batch_processing()
        test_batch.test_batch_output_structure()
    finally:
        test_batch.cleanup()

    print("=" * 50)
    print("ALL TESTS PASSED!")
    print("=" * 50)


if __name__ == "__main__":
    main()
