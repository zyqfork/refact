use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use crate::exec::types::ExecProcessId;

pub struct SpillWriter {
    path: PathBuf,
    file: tokio::fs::File,
}

impl SpillWriter {
    pub async fn create(chat_id: &str, process_id: &ExecProcessId) -> Result<Self, String> {
        let home =
            home::home_dir().ok_or_else(|| "failed to resolve home directory".to_string())?;
        let dir = home
            .join(".cache")
            .join("refact")
            .join("exec")
            .join(chat_id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("failed to create exec spill directory: {error}"))?;
        let path = dir.join(format!("{}.log", process_id.as_str()));
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|error| format!("failed to open exec spill log: {error}"))?;
        Ok(Self { path, file })
    }

    pub async fn write_line(&mut self, line: &str) -> Result<(), String> {
        self.file
            .write_all(line.as_bytes())
            .await
            .map_err(|error| format!("failed to write exec spill log: {error}"))?;
        self.file
            .flush()
            .await
            .map_err(|error| format!("failed to flush exec spill log: {error}"))
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}
