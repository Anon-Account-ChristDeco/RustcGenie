"""Configuration for GPT-OSS-20B client."""

from dataclasses import dataclass


@dataclass
class Config:
    """Configuration for the GPT-OSS-20B client."""

    project_id: str = "YOUR-PROJECT-ID"
    location: str = "us-central1"
    model_id: str = "openai/gpt-oss-20b-maas"
    max_tokens: int = 8192
    temperature: float = 0.7
    top_p: float = 1.0
