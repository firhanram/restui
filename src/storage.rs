use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::Workspace;

const STATE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    version: u32,
    active_workspace: Uuid,
    workspaces: Vec<Workspace>,
}

#[derive(Debug)]
pub struct LoadedState {
    pub workspaces: Vec<Workspace>,
    pub active_workspace: usize,
}

#[derive(Debug)]
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn discover() -> Result<Self> {
        if let Some(path) = std::env::var_os("RESTUI_DATA_DIR") {
            return Ok(Self::new(PathBuf::from(path).join("workspaces.json")));
        }

        let project = ProjectDirs::from("dev", "restui", "restui")
            .context("could not determine the application data directory")?;
        Ok(Self::new(project.data_dir().join("workspaces.json")))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<LoadedState> {
        if !self.path.exists() {
            let workspace = Workspace::new("Default");
            return Ok(LoadedState {
                active_workspace: 0,
                workspaces: vec![workspace],
            });
        }

        let contents = fs::read_to_string(&self.path)
            .with_context(|| format!("could not read {}", self.path.display()))?;
        let state: PersistedState = serde_json::from_str(&contents)
            .with_context(|| format!("could not parse {}", self.path.display()))?;

        if state.version != STATE_VERSION {
            anyhow::bail!("unsupported workspace data version: {}", state.version);
        }
        if state.workspaces.is_empty() {
            anyhow::bail!("workspace data contains no workspaces");
        }

        let active_workspace = state
            .workspaces
            .iter()
            .position(|workspace| workspace.id == state.active_workspace)
            .unwrap_or(0);

        Ok(LoadedState {
            workspaces: state.workspaces,
            active_workspace,
        })
    }

    pub fn save(&self, workspaces: &[Workspace], active_workspace: usize) -> Result<()> {
        if workspaces.is_empty() {
            anyhow::bail!("cannot save an empty workspace list");
        }

        let parent = self
            .path
            .parent()
            .context("workspace data path has no parent directory")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;

        let state = PersistedState {
            version: STATE_VERSION,
            active_workspace: workspaces[active_workspace.min(workspaces.len() - 1)].id,
            workspaces: workspaces.to_vec(),
        };
        let contents = serde_json::to_string_pretty(&state)?;
        let temporary_path = self.path.with_extension("json.tmp");
        fs::write(&temporary_path, contents)
            .with_context(|| format!("could not write {}", temporary_path.display()))?;
        fs::rename(&temporary_path, &self.path).with_context(|| {
            format!(
                "could not replace {} with {}",
                self.path.display(),
                temporary_path.display()
            )
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::model::RequestConfig;

    #[test]
    fn workspaces_round_trip_with_active_selection() {
        let directory = tempdir().unwrap();
        let storage = Storage::new(directory.path().join("nested/workspaces.json"));
        let mut first = Workspace::new("First");
        first.requests.push(RequestConfig::new());
        let second = Workspace::new("Second");

        storage.save(&[first.clone(), second.clone()], 1).unwrap();
        let loaded = storage.load().unwrap();

        assert_eq!(loaded.active_workspace, 1);
        assert_eq!(loaded.workspaces.len(), 2);
        assert_eq!(loaded.workspaces[0].name, "First");
        assert_eq!(loaded.workspaces[0].requests.len(), 2);
        assert_eq!(loaded.workspaces[1].id, second.id);
    }

    #[test]
    fn missing_file_creates_default_workspace() {
        let directory = tempdir().unwrap();
        let storage = Storage::new(directory.path().join("workspaces.json"));
        let loaded = storage.load().unwrap();

        assert_eq!(loaded.workspaces.len(), 1);
        assert_eq!(loaded.workspaces[0].name, "Default");
        assert_eq!(loaded.workspaces[0].requests.len(), 1);
    }
}
