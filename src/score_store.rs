use crate::resources::HighScores;
use bevy::prelude::*;
use std::{io, path::PathBuf};

/// Local, editable JSON file, relative to the game's working directory.
#[derive(Resource)]
pub struct HighScoreStore(pub PathBuf);

impl Default for HighScoreStore {
    fn default() -> Self {
        Self(PathBuf::from("saves/high_scores.json"))
    }
}

impl HighScoreStore {
    pub fn load(&self) -> io::Result<HighScores> {
        #[cfg(not(target_arch = "wasm32"))]
        let bytes = match std::fs::read(&self.0) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(HighScores::default());
            }
            Err(error) => return Err(error),
        };
        #[cfg(target_arch = "wasm32")]
        let bytes = match browser_storage()?
            .get_item("soup-master.high-scores.v1")
            .map_err(|error| io::Error::other(format!("{error:?}")))?
        {
            Some(json) => json.into_bytes(),
            None => return Ok(HighScores::default()),
        };
        let mut scores: HighScores = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        for (name, _) in &mut scores.entries {
            *name = clean_name(name);
        }
        scores
            .entries
            .retain(|(name, score)| !name.is_empty() && *score > 0);
        scores.entries.sort_by(|a, b| b.1.cmp(&a.1));
        scores.entries.truncate(5);
        Ok(scores)
    }

    /// Write first, then replace the old file so an interrupted write cannot truncate it.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, scores: &HighScores) -> io::Result<()> {
        if let Some(parent) = self.0.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = self.0.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(scores).map_err(io::Error::other)?;
        let mut file = std::fs::File::create(&temporary)?;
        use std::io::Write;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(temporary, &self.0)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(&self, scores: &HighScores) -> io::Result<()> {
        let json = serde_json::to_string(scores).map_err(io::Error::other)?;
        browser_storage()?
            .set_item("soup-master.high-scores.v1", &json)
            .map_err(|error| io::Error::other(format!("{error:?}")))
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_storage() -> io::Result<web_sys::Storage> {
    web_sys::window()
        .ok_or_else(|| io::Error::other("Browser window unavailable"))?
        .local_storage()
        .map_err(|error| io::Error::other(format!("{error:?}")))?
        .ok_or_else(|| io::Error::other("Browser local storage unavailable"))
}

pub fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || " -_!*".contains(*c))
        .take(10)
        .collect::<String>()
        .trim()
        .to_ascii_uppercase()
}

pub fn load_high_scores(mut scores: ResMut<HighScores>, store: Res<HighScoreStore>) {
    match store.load() {
        Ok(saved) => *scores = saved,
        Err(error) => warn!(
            "Cannot load high scores from {}: {error}",
            store.0.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_scores_survive_reload_and_replace_the_previous_file() {
        let folder = std::env::temp_dir().join(format!(
            "soup-master-scores-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = HighScoreStore(folder.join("high_scores.json"));
        assert!(store.load().unwrap().entries.is_empty());
        let mut scores = HighScores {
            entries: vec![("CHEF".into(), 3)],
        };
        store.save(&scores).unwrap();
        assert_eq!(store.load().unwrap().entries, scores.entries);
        scores.entries.push(("SOUP".into(), 9));
        store.save(&scores).unwrap();
        assert_eq!(
            store.load().unwrap().entries,
            vec![("SOUP".into(), 9), ("CHEF".into(), 3)]
        );
        std::fs::write(&store.0, "invalid json").unwrap();
        assert!(store.load().is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
