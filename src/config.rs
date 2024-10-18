use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub db_path: PathBuf,
    pub basic: Basic,

    #[serde(default)]
    pub rule: Vec<Rule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Basic {
    pub ignore: HashSet<String>,
    pub link_ext: HashSet<String>,
    pub copy_ext: HashSet<String>,
    #[cfg(unix)]
    pub uid: Option<u32>,
    #[cfg(unix)]
    pub gid: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub src: PathBuf,
    pub target: PathBuf,
}
