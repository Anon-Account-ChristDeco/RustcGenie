"""GPT-OSS-20B Google Cloud client using Vertex AI MaaS."""

import openai
from openai.types.chat.chat_completion import ChatCompletion
from google.auth import default
from google.auth.transport import requests

from config import Config


class GPTOSSClient:
    """Client for calling GPT-OSS-20B via Google Cloud Vertex AI."""

    def __init__(self, config: Config | None = None):
        self.config = config or Config()

    def _get_gcp_token(self) -> str:
        """Obtain GCP token using Application Default Credentials."""
        credentials, _ = default()
        auth_request = requests.Request()
        credentials.refresh(auth_request)
        return credentials.token

    def _get_endpoint_url(self) -> str:
        """Construct the Vertex AI MaaS endpoint URL."""
        return (
            f"https://{self.config.location}-aiplatform.googleapis.com/v1beta1/"
            f"projects/{self.config.project_id}/locations/{self.config.location}/endpoints/openapi"
        )

    def query(self, prompt: str) -> ChatCompletion:
        """Send a prompt to the model and return the completion."""
        client = openai.OpenAI(
            base_url=self._get_endpoint_url(),
            api_key=self._get_gcp_token(),
        )

        messages = [
            {
                "role": "user",
                "content": [{"type": "text", "text": prompt}],
            }
        ]

        response = client.chat.completions.create(
            model=self.config.model_id,
            messages=messages,
            max_tokens=self.config.max_tokens,
            temperature=self.config.temperature,
            top_p=self.config.top_p,
        )

        return response

    def get_text(self, prompt: str) -> str | None:
        """Send a prompt and return only the generated text."""
        response = self.query(prompt)
        return response.choices[0].message.content


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 3:
        print("Usage: python client.py -t <text prompt> [--all]")
        print("   or: python client.py -f <prompt file> [--all]")
        sys.exit(1)

    prompt = ""
    show_all = "--all" in sys.argv

    if sys.argv[1] == "-t":
        prompt = sys.argv[2]
    elif sys.argv[1] == "-f":
        with open(sys.argv[2], "r", encoding="utf-8") as f:
            prompt = f.read()
    else:
        print("Invalid option. Use -t for text or -f for file.")
        sys.exit(1)

    client = GPTOSSClient()
    response = client.query(prompt)

    if show_all:
        print(response)
    else:
        print(response.choices[0].message.content)
