use std::fmt;
use std::fs;
use std::io::Write;
use std::process::Command;

#[derive(Debug)]
pub enum LlmError {
    Io(std::io::Error),
    NonZeroExit { code: Option<i32>, stderr: String },
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::Io(e) => write!(f, "IO error: {e}"),
            LlmError::NonZeroExit { code, stderr } => {
                write!(f, "LLM client exited with code {code:?}: {stderr}")
            }
        }
    }
}

impl std::error::Error for LlmError {}

impl From<std::io::Error> for LlmError {
    fn from(e: std::io::Error) -> Self {
        LlmError::Io(e)
    }
}

/// Resolve the path to the Python LLM client script.
///
/// Checks `RUSTC_FREEZER_ROOT` env var first, then falls back to
/// navigating from `CARGO_MANIFEST_DIR` (i.e. `../../vendor/gpt-oss-20b/client.py`).
fn client_script_path() -> std::path::PathBuf {
    if let Ok(root) = std::env::var("RUSTC_FREEZER_ROOT") {
        return std::path::PathBuf::from(root).join("vendor/gpt-oss-20b/client.py");
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/gpt-oss-20b/client.py")
}

/// Call the LLM by writing the prompt to a temp file and shelling out
/// to `python3 vendor/gpt-oss-20b/client.py -f <tmpfile>`.
///
/// Returns the raw stdout (the LLM response text).
pub fn call_llm(prompt: &str) -> Result<String, LlmError> {
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("llm-extractor-prompt-{}.txt", std::process::id()));

    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(prompt.as_bytes())?;
    file.flush()?;
    drop(file);

    let script = client_script_path();
    let output = Command::new("python3")
        .arg(&script)
        .arg("-f")
        .arg(&tmp_path)
        .output()?;

    // Best-effort cleanup
    let _ = fs::remove_file(&tmp_path);

    if !output.status.success() {
        return Err(LlmError::NonZeroExit {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
