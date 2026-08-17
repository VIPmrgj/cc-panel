use std::path::{Path, PathBuf};

use crate::dto::{ApiError, ApiResult};

#[derive(Debug, Clone)]
pub struct ClaudePaths {
    home: PathBuf,
    user_claude_dir: PathBuf,
    user_settings: PathBuf,
    user_skills: PathBuf,
}

impl ClaudePaths {
    pub fn detect() -> ApiResult<Self> {
        let home = dirs::home_dir().ok_or_else(|| {
            ApiError::new(
                "HOME_DIRECTORY_UNAVAILABLE",
                "无法确定当前用户主目录。",
                false,
            )
        })?;
        Ok(Self::from_home(home))
    }

    pub fn from_home(home: PathBuf) -> Self {
        let user_claude_dir = home.join(".claude");
        Self {
            user_settings: user_claude_dir.join("settings.json"),
            user_skills: user_claude_dir.join("skills"),
            home,
            user_claude_dir,
        }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn user_claude_dir(&self) -> &Path {
        &self.user_claude_dir
    }

    pub fn user_settings(&self) -> &Path {
        &self.user_settings
    }

    pub fn user_skills(&self) -> &Path {
        &self.user_skills
    }
}
