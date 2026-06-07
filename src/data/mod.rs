use std::fs::File;

use directories::{BaseDirs, ProjectDirs};
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Serialize, Deserialize)]
pub struct Data {
    pub selected_entry_dir: String,
    pub custom_artifacts: Vec<String>,
}

impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}

impl Data {
    fn new() -> Data {
        // home directory is the default directory if its not explicitly set
        // in the program, "." is used as fallback to work with the current directory
        // if home dir resolution fails somehow.
        let default_dir = if let Some(base_dir) = BaseDirs::new() {
            let base = base_dir.home_dir().to_str();
            base.unwrap_or(".").to_string()
        } else {
            ".".to_string()
        };

        // default data
        let mut data = Data {
            selected_entry_dir: default_dir,
            custom_artifacts: vec![],
        };

        if let Some(saved) = Self::load_saved_config() {
            data = saved;
        }

        data
    }

    fn load_saved_config() -> Option<Data> {
        // to make the config file save across all OS i.e. works on linux, macOs, and windows
        let project_dirs = ProjectDirs::from("com", "devtox", "devtox");

        // the config directory path
        let config_dir = if let Some(dir) = &project_dirs {
            dir.config_dir()
        } else {
            return None;
        };

        let config_file_path = config_dir.join("config.json");

        let config_file = match File::open(config_file_path) {
            Ok(f) => f,
            Err(e) => {
                error!("{}", e);
                return None;
            }
        };

        match serde_json::from_reader(config_file) {
            Ok(d) => Some(d),
            Err(e) => {
                error!("Failed to deserialize config: {}", e);
                None
            }
        }
    }
}
