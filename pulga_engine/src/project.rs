//! A project: everything the editor saves for one game.
//!
//! On disk a project is a directory holding [`PROJECT_FILE`] (TOML, so it
//! reads and diffs well in git) next to the game's own Rust crate. The game
//! embeds the file at build time:
//!
//! ```ignore
//! let project = Project::from_toml(include_str!("../project.toml"))?;
//! ```
//!
//! ```
//! use pulga_engine::project::Project;
//!
//! let project = Project::starter("demo");
//! let text = project.to_toml();
//! assert_eq!(Project::from_toml(&text).unwrap(), project);
//! ```

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::tile::{Prop, PropKind, Tile, TileKind};

/// File name of a project inside its directory.
pub const PROJECT_FILE: &str = "project.toml";

/// See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// Display name.
    pub name: String,
    /// The building blocks runs are generated from.
    #[serde(default)]
    pub tiles: Vec<Tile>,
}

/// Why a project failed to load or save.
#[derive(Debug)]
pub enum ProjectError {
    /// Reading or writing the file failed.
    Io(std::io::Error),
    /// The file isn't a valid project.
    Parse(toml::de::Error),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "invalid {PROJECT_FILE}: {e}"),
        }
    }
}

impl std::error::Error for ProjectError {}

const OBSTACLE_COLOR: [f32; 3] = [0.8, 0.25, 0.2];

impl Project {
    /// A project to start from: one tile of every kind, plus straights with
    /// an obstacle in each lane (lanes at x = -2.2, 0, 2.2).
    pub fn starter(name: impl Into<String>) -> Self {
        let mut tiles: Vec<Tile> = TileKind::ALL
            .iter()
            .map(|&k| Tile::new(k.name(), k))
            .collect();
        for (lane, x) in [("left", -2.2), ("middle", 0.0), ("right", 2.2)] {
            let mut tile = Tile::new(format!("block-{lane}"), TileKind::Straight);
            tile.props.push(Prop {
                kind: PropKind::Obstacle,
                position: [x, 0.5, -5.0],
                size: [1.0, 1.0, 1.0],
                color: OBSTACLE_COLOR,
            });
            tiles.push(tile);
        }
        Self {
            name: name.into(),
            tiles,
        }
    }

    /// Parses a project from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        toml::from_str(text).map_err(ProjectError::Parse)
    }

    /// Serializes the project to TOML text.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("projects only hold TOML-friendly data")
    }

    /// Loads `dir`/[`PROJECT_FILE`].
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let text =
            std::fs::read_to_string(dir.as_ref().join(PROJECT_FILE)).map_err(ProjectError::Io)?;
        Self::from_toml(&text)
    }

    /// Writes `dir`/[`PROJECT_FILE`], creating `dir` if needed.
    pub fn save(&self, dir: impl AsRef<Path>) -> Result<(), ProjectError> {
        std::fs::create_dir_all(dir.as_ref()).map_err(ProjectError::Io)?;
        std::fs::write(dir.as_ref().join(PROJECT_FILE), self.to_toml()).map_err(ProjectError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("pulga-project-test-{}", std::process::id()));
        let project = Project::starter("round trip");
        project.save(&dir).unwrap();
        let loaded = Project::load(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(loaded, project);
    }

    #[test]
    fn missing_fields_default_and_bad_text_errors() {
        let p =
            Project::from_toml("name = \"x\"\n[[tiles]]\nname = \"t\"\nkind = \"left\"\n").unwrap();
        assert!(p.tiles[0].props.is_empty());
        assert!(matches!(
            Project::from_toml("name = 3"),
            Err(ProjectError::Parse(_))
        ));
    }

    #[test]
    fn starter_has_every_kind_and_some_obstacles() {
        let p = Project::starter("s");
        for kind in TileKind::ALL {
            assert!(p.tiles.iter().any(|t| t.kind == kind), "{kind:?}");
        }
        let obstacles = p
            .tiles
            .iter()
            .flat_map(|t| &t.props)
            .filter(|prop| prop.kind == PropKind::Obstacle)
            .count();
        assert_eq!(obstacles, 3);
    }
}
