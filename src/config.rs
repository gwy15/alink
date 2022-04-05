use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub basic: Basic,

    pub rule: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
pub struct Basic {
    pub db_path: PathBuf,
    #[serde(default = "media_ext_default")]
    pub media_ext: HashSet<String>,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub src: PathBuf,
    pub target: PathBuf,
}

fn media_ext_default() -> HashSet<String> {
    ["mp4", "mkv", "avi", "flv", "ts", "mov", "wmv", "webm"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}
