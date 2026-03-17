"""Test in-Python single call usage"""

import sys
sys.path.insert(0, ".")

from client import GPTOSSClient
from config import Config


def test_default_config():
    """Test GPTOSSClient with default config"""
    print("=== Python API Default Config Test ===")

    client = GPTOSSClient()
    response = client.query("What is 2+2? Answer with just the number.")

    print(f"Model: {response.model}")
    print(f"Usage: {response.usage}")
    print(f"Content: {response.choices[0].message.content}")

    assert response.choices[0].message.content is not None, "No content in response"
    print("PASSED\n")


def test_custom_config():
    """Test GPTOSSClient with custom config"""
    print("=== Python API Custom Config Test ===")

    custom_config = Config(
        temperature=0.3,
        max_tokens=100,
    )
    client = GPTOSSClient(custom_config)
    response = client.query("Say 'test' and nothing else.")

    print(f"Temperature: {custom_config.temperature}")
    print(f"Max tokens: {custom_config.max_tokens}")
    print(f"Content: {response.choices[0].message.content}")

    assert response.choices[0].message.content is not None, "No content in response"
    print("PASSED\n")


def test_get_text_method():
    """Test the get_text convenience method"""
    print("=== Python API get_text() Test ===")

    client = GPTOSSClient()
    text = client.get_text("Reply with only the word 'hello'")

    print(f"Returned text: {text}")

    assert text is not None, "get_text returned None"
    assert isinstance(text, str), "get_text should return string"
    print("PASSED\n")


if __name__ == "__main__":
    test_default_config()
    test_custom_config()
    test_get_text_method()
    print("All Python API tests passed!")
