use std::{fs::File, io::Write, path::PathBuf};

use directories::{BaseDirs, ProjectDirs};
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Serialize, Deserialize, Debug)]
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
        } else {
            // config file prolly doesn't exist, so create one using the default data values
            data.save_config();
        }

        data
    }

    fn get_config_path() -> Option<PathBuf> {
        // to make the config file save across all OS i.e. works on linux, macOs, and windows
        let project_dirs = ProjectDirs::from("com", "devtox", "devtox");

        // the config directory path
        if let Some(dir) = &project_dirs {
            let config_dir = dir.config_dir();
            Some(config_dir.join("config.json"))
        } else {
            None
        }
    }

    fn load_saved_config() -> Option<Data> {
        let config_file_path = Self::get_config_path()?;

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

    // to sync data to file after updating the Data struct
    fn save_config(&self) {
        if let Some(config_file_path) = Self::get_config_path() {
            // create parent directories if they don't exist
            if let Some(parent) = config_file_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    error!("Failed to create config directory: {}", e);
                    return;
                }
            }

            // creates the file if it doesn't exist, otherwise write to it
            match File::create(config_file_path) {
                // stringifies the in-memory data struct and writes it to config file
                Ok(mut f) => match serde_json::to_string(self) {
                    Ok(stringified) => {
                        if let Err(e) = f.write_all(stringified.as_bytes()) {
                            error!("Failed to write config: {}", e);
                        }
                    }
                    Err(e) => error!("Failed to serialize config: {}", e),
                },
                Err(e) => error!("Failed to open/create config file: {}", e),
            };
        } else {
            error!("Can't get config_file path");
            panic!(
                "Couldn't update the config due to error handling missing config file. Running the program otherwise would've possibly used stale paths and caused unexpected directories to be deleted."
            )
        };
    }

    pub fn update_dir(&mut self, dir: String) {
        self.selected_entry_dir = dir;
        self.save_config();
    }

    pub fn update_artifacts(&mut self, artifacts: Vec<String>) {
        self.custom_artifacts = artifacts;
        self.save_config();
    }
}
