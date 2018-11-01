// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Checkpoints management routines

use std::{
    collections::{BTreeMap, HashMap},
    error::Error as StdError,
    fmt,
    fs::{create_dir_all, remove_dir_all, remove_file, rename, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use crypto::PublicKey;
use helpers::Height;
use messages::{FileResponse, LastCheckpointResponse};
use serde_json::{from_str, to_string};
use storage::Database;

/// Structure that is holding information about created and received checkpoints
pub struct CheckpointManager {
    db: Arc<dyn Database>,
    validator_pk: PublicKey,
    checkpoints_received: BTreeMap<PublicKey, Vec<CheckpointState>>,
    pub checkpoint_period_blocks: Option<u64>,
    max_checkpoints: Option<usize>,
    last_checkpoint: Option<CheckpointPointer>,
}

/// JSON representation of list of files.
pub type FilesList = String;

/// Pointer to the checkpoint.
pub type CheckpointPointer = (Height, PathBuf, FilesList);

/// State of a checkpoint reception.
#[derive(Debug, Clone)]
pub struct CheckpointState {
    /// Height on which checkpoint was created
    pub height: Height,
    /// Name of checkpoint.
    pub name: String,
    /// Set of files to download. Value `true` is for received files.
    pub downloaded_files: HashMap<String, bool>,
}

impl CheckpointManager {
    /// Creates new instance of CheckpointManager with checkpoint creation period and maximum number
    /// of checkpoints.
    pub fn new<D: Into<Arc<dyn Database>>>(
        db: D,
        validator_pk: PublicKey,
        checkpoint_period_blocks: Option<u64>,
        max_checkpoints: Option<usize>,
    ) -> Self {
        let mut checkpoints = Self {
            db: db.into(),
            validator_pk,
            checkpoints_received: BTreeMap::new(),
            checkpoint_period_blocks,
            max_checkpoints,
            last_checkpoint: None,
        };

        let last_checkpoint = checkpoints
            .list_checkpoints()
            .unwrap_or(vec![])
            .last()
            .map(|x| x.clone());
        checkpoints.last_checkpoint = last_checkpoint;

        if let Some(ref lc) = checkpoints.last_checkpoint {
            info!("Last checkpoint is: ({}, {})", lc.0, lc.1.to_string_lossy());
        }

        checkpoints
    }

    /// Returns last created checkpoint pointer (if any).
    pub fn last_checkpoint(&self) -> Option<CheckpointPointer> {
        self.last_checkpoint.clone()
    }

    /// Adds response that is received from peer to current state.
    pub fn add_checkpoint(&mut self, checkpoint: &LastCheckpointResponse) {
        // Don't add checkpoints that are the same as or older than actual checkpoint
        {
            let most_actual = self.most_actual_checkpoint(checkpoint.from());
            if let Some(most_actual) = most_actual {
                if most_actual.name == *checkpoint.name()
                    && most_actual.height >= checkpoint.height()
                {
                    return;
                }
            }
        }

        let mut downloaded_files = HashMap::new();
        let files: Vec<String> = from_str(checkpoint.files()).unwrap_or(vec![]);
        for file in files.iter() {
            downloaded_files.insert(file.clone(), false);
        }

        if files.is_empty() || checkpoint.files().is_empty() {
            warn!(
                "[checkpoint] Received empty or invalid checkpoint from {:?}",
                checkpoint.from()
            );
            return;
        }

        let checkpoint_state = CheckpointState {
            height: checkpoint.height(),
            name: checkpoint.name().to_string(),
            downloaded_files,
        };

        info!(
            "[checkpoint] Added new checkpoint from {} with name {} and {} files",
            checkpoint.from(),
            checkpoint.name(),
            files.len()
        );

        if let Some(vec) = self.checkpoints_received.get_mut(&*checkpoint.from()) {
            vec.push(checkpoint_state);
            return;
        }

        self.checkpoints_received
            .insert(*checkpoint.from(), vec![checkpoint_state]);
    }

    fn most_actual_checkpoint(&mut self, peer: &PublicKey) -> Option<&mut CheckpointState> {
        if let Some(checkpoints) = self.checkpoints_received.get_mut(peer) {
            // Search for the most actual checkpoint
            let mut most_actual = None;
            let mut max_height = None;
            for mut cp in checkpoints.iter_mut() {
                if let Some(height) = max_height {
                    if cp.height > height {
                        max_height = Some(cp.height);
                        most_actual = Some(cp);
                    }
                } else {
                    max_height = Some(cp.height);
                    most_actual = Some(cp);
                }
            }

            return most_actual;
        }

        None
    }

    /// Returns request for the next file from currently downloading checkpoint
    pub fn next_checkpoint_file_request(&mut self, peer: &PublicKey) -> Option<(String, String)> {
        if let Some(checkpoint) = self.most_actual_checkpoint(peer) {
            let (total, downloaded) = {
                // Count downloaded and total number of files
                let total = checkpoint.downloaded_files.len();
                let downloaded = checkpoint
                    .downloaded_files
                    .iter()
                    .filter(|(_, is_downloaded)| **is_downloaded)
                    .count();

                (total, downloaded)
            };

            for (file_name, is_downloaded) in checkpoint.downloaded_files.iter() {
                if !is_downloaded {
                    info!(
                        "[checkpoint] Requesting {} file {}/{}",
                        file_name, downloaded, total
                    );

                    return Some((checkpoint.name.clone(), file_name.clone()));
                }
            }
        } else {
            warn!("[checkpoint] Was unable to find most actual checkpoint!");
            return None;
        }

        info!("[checkpoint] All files are downloaded!");

        None
    }

    fn checkpoint_by_name(&mut self, from: &PublicKey, name: &str) -> Option<&mut CheckpointState> {
        if let Some(checkpoints) = self.checkpoints_received.get_mut(from) {
            for cp in checkpoints {
                if cp.name == name {
                    return Some(cp);
                }
            }
        }

        None
    }

    /// Saves file into corresponding checkpoint
    fn write_checkpoint_file(
        &self,
        checkpoint_name: &str,
        file_name: &str,
        data: &[u8],
    ) -> Result<(), Box<dyn StdError>> {
        let checkpoint_name = Path::new(checkpoint_name);

        let mut checkpoints_dir_path = PathBuf::new();
        checkpoints_dir_path.push("checkpoints");
        checkpoints_dir_path.push("received");
        checkpoints_dir_path.push(&self.validator_pk.to_hex());
        checkpoints_dir_path.push(checkpoint_name);

        if !checkpoints_dir_path.exists() {
            create_dir_all(&checkpoints_dir_path)?;
        }

        let mut file_path = checkpoints_dir_path.clone();
        file_path.push(file_name);

        if file_path.exists() {
            remove_file(&file_path)?;
        }

        let mut file = File::create(file_path)?;
        file.write_all(data)?;
        file.flush()?;

        Ok(())
    }

    /// Saves received file for checkpoint.
    /// Returns `true` if all files were downloaded and checkpoint is ready to be applied.
    pub fn save_checkpoint_file(&mut self, response: &FileResponse) -> bool {
        let file_name = response.file_name().clone();
        let checkpoint_name = response.checkpoint_name();
        let mut all_downloaded = true;

        if let Some(checkpoint) = self.checkpoint_by_name(&response.from(), checkpoint_name) {
            if let Some(mut is_downloaded) = checkpoint.downloaded_files.get_mut(file_name) {
                *is_downloaded = true;
            }

            // Check whether there are more files to download or not
            for (_, is_downloaded) in checkpoint.downloaded_files.iter() {
                if !is_downloaded {
                    all_downloaded = false;
                }
            }
        }

        if let Err(e) = self.write_checkpoint_file(&checkpoint_name, file_name, response.data()) {
            error!("Was unable to save checkpoint file {}: {}", file_name, e);
        }

        all_downloaded
    }

    /// Sets period in blocks of creation of DB checkpoint.
    /// Pass `None` to disable checkpoint creation.
    pub fn set_checkpoint_period_blocks(&mut self, period: Option<u64>) {
        self.checkpoint_period_blocks = period;
    }

    /// Sets maximum number of checkpoints to create. When maximum is reached, oldest checkpoint
    /// will be removed thus preserving ring or checkpoints of `max_checkpoints` length.
    /// Pass `None` to allow unlimited number of checkpoints.
    pub fn set_max_checkpoints(&mut self, max_checkpoints: Option<usize>) {
        self.max_checkpoints = max_checkpoints;
    }

    /// Removes checkpoints to not exceed `max_checkpoints` number.
    fn purge_checkpoints(&self) -> Result<(), Box<dyn StdError>> {
        if let Some(max_checkpoints) = self.max_checkpoints {
            if max_checkpoints <= 1 {
                warn!(
                    "Invalid max_checkpoints value: {}, it can't be lower than 2",
                    max_checkpoints
                );
                return Ok(());
            }

            let mut dir_path = PathBuf::new();
            dir_path.push("checkpoints");
            dir_path.push("created");

            let checkpoints = self.list_checkpoints()?;
            if checkpoints.len() > max_checkpoints {
                let num_to_purge = checkpoints.len() - max_checkpoints;
                let to_purge: Vec<_> = checkpoints.iter().take(num_to_purge).collect();

                info!("Purging {} checkpoints, {:#?}", num_to_purge, to_purge);

                for (_, path, _) in to_purge {
                    let path = dir_path.as_path().join(&path);

                    info!("Purging excess checkpoint {}", path.to_string_lossy());
                    remove_dir_all(path)?;
                }
            }
        }

        Ok(())
    }

    fn list_files_in_path<P: AsRef<Path>>(path: P) -> String {
        let files_list = path
            .as_ref()
            .read_dir()
            .map(|rd| {
                rd.map(|i| {
                    i.map(|de| {
                        de.path()
                            .file_name()
                            .map(|oss| oss.to_str().unwrap_or("").to_string())
                            .unwrap_or("".to_string())
                    }).unwrap_or("".to_string())
                }).collect::<Vec<_>>()
            }).unwrap_or(vec![]);

        // Convert into JSON representation
        to_string(&files_list).unwrap_or("".to_string())
    }

    /// Returns list of checkpoints created by node sorted by height ascending.
    fn list_checkpoints(&self) -> Result<Vec<CheckpointPointer>, Box<dyn StdError>> {
        let mut checkpoints_dir_path = PathBuf::new();
        checkpoints_dir_path.push("checkpoints");
        checkpoints_dir_path.push("created");
        checkpoints_dir_path.push(self.validator_pk.to_hex());

        if !checkpoints_dir_path.exists() {
            return Ok(vec![]);
        }

        let checkpoints = checkpoints_dir_path.read_dir()?;
        let mut checkpoints = checkpoints
            .map(|rd| rd.map(|i| i.path()).unwrap())
            .map(|pb| {
                (
                    pb.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .parse::<u64>()
                        .unwrap(),
                    pb,
                )
            }).map(|(h, pb)| {
                let files_list = CheckpointManager::list_files_in_path(&pb);

                // Strip prefix from path
                let mut pb = PathBuf::new();
                pb.push(&self.validator_pk.to_hex());
                pb.push(h.to_string());

                (Height(h), pb, files_list)
            }).collect::<Vec<_>>();

        checkpoints.sort_unstable_by(|(h1, ..), (h2, ..)| h1.cmp(h2));

        Ok(checkpoints)
    }

    /// Creates a new DB checkpoint.
    pub fn create_checkpoint(&mut self, height: &Height) -> Result<(), Box<dyn StdError>> {
        // Remove some excess checkpoints prior checkpoint creation
        if let Err(e) = self.purge_checkpoints() {
            error!("Was unable to purge checkpoints: {}", e);
        }

        let mut checkpoints_dir_path = PathBuf::new();
        checkpoints_dir_path.push("checkpoints");
        checkpoints_dir_path.push("created");
        checkpoints_dir_path.push(&self.validator_pk.to_hex());

        if !checkpoints_dir_path.exists() {
            create_dir_all(&checkpoints_dir_path)?;
        }

        info!("List of checkpoints: {:#?}", self.list_checkpoints()?);
        info!("Creating checkpoint for height: {}", height);

        let mut cp_path = checkpoints_dir_path.clone();
        cp_path.push(height.to_string());

        let mut path_temp = checkpoints_dir_path.clone();
        path_temp.push(format!("{}_tmp", height));

        if path_temp.exists() {
            remove_dir_all(&path_temp)?;
        }

        if self
            .db
            .create_checkpoint(&path_temp.to_str().unwrap())
            .is_ok()
        {
            // Strip prefix from path
            let mut path = PathBuf::new();
            path.push(&self.validator_pk.to_hex());
            path.push(height.to_string());

            // Collect list of files in directory
            let files_list = CheckpointManager::list_files_in_path(&path_temp);

            self.last_checkpoint = Some((*height, path, files_list));
        }

        if cp_path.exists() {
            remove_dir_all(&cp_path)?;
        }

        // Move temporary checkpoint into current
        rename(&path_temp, &cp_path)?;

        info!("Checkpoint {} has been created", &cp_path.to_str().unwrap());

        // Remove checkpoint that is become excess after checkpoint is created
        let _ = self.purge_checkpoints();

        Ok(())
    }

    /// Reads file form the given checkpoint and returns its content
    pub fn read_checkpoint_file(
        &self,
        checkpoint_name: &str,
        file_name: &str,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        use std::io::{BufReader, Read};

        let checkpoint_name = Path::new(checkpoint_name);

        let mut path = PathBuf::new();
        path.push("checkpoints");
        path.push("created");
        path.push(checkpoint_name);
        path.push(file_name);

        let file = File::open(&path)?;
        let mut reader = BufReader::new(file);

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;

        Ok(bytes)
    }

    /// Applies received checkpoint by name.
    pub fn apply_checkpoint(&mut self, checkpoint_name: &str) -> Result<(), Box<dyn StdError>> {
        info!("Applying checkpoint {}...", checkpoint_name);

        let mut path = PathBuf::new();
        path.push("checkpoints");
        path.push("received");
        path.push(&self.validator_pk.to_hex());
        path.push(checkpoint_name);

        let _ = self.db.apply_checkpoint(&path.to_string_lossy());

        Ok(())
    }
}

impl fmt::Debug for CheckpointManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CheckpointManager(..)")
    }
}
