use std::collections::BTreeMap;
use std::path::Path;

use crate::profile::types::{ActorProfile, ActorTier};
use crate::TianJiError;

/// Registry of actor profiles loaded from YAML files.
/// Maps profile `id` to `ActorProfile`, stored in a `BTreeMap` for deterministic ordering.
#[derive(Clone, Debug, Default)]
pub struct ProfileRegistry {
    pub profiles: BTreeMap<String, ActorProfile>,
}

impl ProfileRegistry {
    /// Load all actor profiles from a directory structure:
    /// ```text
    /// <path>/
    /// ├── nations/
    /// │   └── *.yaml / *.yml
    /// ├── organizations/
    /// │   └── *.yaml / *.yml
    /// └── corporations/
    ///     └── *.yaml / *.yml
    /// ```
    ///
    /// Each YAML file must deserialize to `ActorProfile`.
    /// Returns an error if the base directory does not exist or any YAML file fails to parse.
    pub fn load_from_dir(path: &Path) -> Result<Self, TianJiError> {
        let mut registry = ProfileRegistry::default();

        if !path.exists() {
            return Err(TianJiError::Input(format!(
                "Profile directory does not exist: {}",
                path.display()
            )));
        }

        for subdir in &["nations", "organizations", "corporations"] {
            let dir = path.join(subdir);
            if !dir.is_dir() {
                continue;
            }
            Self::load_yaml_files_from_dir(&dir, &mut registry)?;
        }

        Ok(registry)
    }

    /// Recursively load `.yaml` / `.yml` files from a single directory.
    fn load_yaml_files_from_dir(
        dir: &Path,
        registry: &mut ProfileRegistry,
    ) -> Result<(), TianJiError> {
        let entries = std::fs::read_dir(dir).map_err(|e| {
            TianJiError::Input(format!(
                "Cannot read profile directory {}: {e}",
                dir.display()
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                TianJiError::Input(format!(
                    "Cannot read directory entry in {}: {e}",
                    dir.display()
                ))
            })?;
            let path = entry.path();

            if path.is_dir() {
                Self::load_yaml_files_from_dir(&path, registry)?;
                continue;
            }

            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if extension != "yaml" && extension != "yml" {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            let profile: ActorProfile = serde_yaml::from_str(&content)
                .map_err(|e| TianJiError::Yaml(e, path.display().to_string()))?;
            registry.profiles.insert(profile.id.clone(), profile);
        }

        Ok(())
    }

    /// Look up a profile by its `id`.
    pub fn get(&self, id: &str) -> Option<&ActorProfile> {
        self.profiles.get(id)
    }

    /// Return all profiles matching a given tier.
    pub fn of_tier(&self, tier: ActorTier) -> Vec<&ActorProfile> {
        self.profiles.values().filter(|p| p.tier == tier).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::types::ActorTier;

    fn write_temp_profiles(base: &Path, files: &[(&str, &str)]) {
        for (relative_path, content) in files {
            let full_path = base.join(relative_path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&full_path, content).unwrap();
        }
    }

    #[test]
    fn load_from_dir_reads_nations_organizations_corporations() {
        let base = tempfile_path("load_all");
        let _ = std::fs::remove_dir_all(&base);
        write_temp_profiles(
            &base,
            &[
                (
                    "nations/china.yaml",
                    r#"
id: china
name: China
tier: nation
interests:
  - goal: "territorial integrity"
    salience: 0.95
red_lines:
  - "foreign military presence"
capabilities:
  military: 0.85
  economic: 0.80
  technological: 0.70
  diplomatic: 0.75
  cyber: 0.82
behavior_patterns: []
historical_analogues: []
"#,
                ),
                (
                    "organizations/nato.yaml",
                    r#"
id: nato
name: NATO
tier: organization
interests:
  - goal: "collective defense"
    salience: 0.90
red_lines:
  - "armed attack on member"
capabilities:
  military: 0.0
  economic: 0.60
  technological: 0.70
  diplomatic: 0.85
  cyber: 0.65
behavior_patterns: []
historical_analogues: []
"#,
                ),
                (
                    "corporations/huawei.yaml",
                    r#"
id: huawei
name: Huawei
tier: corporation
interests:
  - goal: "market share"
    salience: 0.85
red_lines:
  - "forced technology transfer"
capabilities:
  military: 0.0
  economic: 0.70
  technological: 0.90
  diplomatic: 0.30
  cyber: 0.75
behavior_patterns: []
historical_analogues: []
"#,
                ),
            ],
        );

        let registry = ProfileRegistry::load_from_dir(&base).unwrap();
        assert_eq!(registry.profiles.len(), 3);

        let china = registry.get("china").unwrap();
        assert_eq!(china.tier, ActorTier::Nation);
        assert!((china.capabilities.military - 0.85).abs() < f64::EPSILON);

        let nato = registry.get("nato").unwrap();
        assert_eq!(nato.tier, ActorTier::Organization);
        assert!((nato.capabilities.military).abs() < f64::EPSILON);

        let huawei = registry.get("huawei").unwrap();
        assert_eq!(huawei.tier, ActorTier::Corporation);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn of_tier_filters_correctly() {
        let base = tempfile_path("tier_filter");
        let _ = std::fs::remove_dir_all(&base);
        write_temp_profiles(
            &base,
            &[
                (
                    "nations/china.yaml",
                    r#"
id: china
name: China
tier: nation
capabilities:
  military: 0.8
  economic: 0.7
  technological: 0.6
  diplomatic: 0.7
  cyber: 0.8
"#,
                ),
                (
                    "nations/usa.yaml",
                    r#"
id: usa
name: USA
tier: nation
capabilities:
  military: 0.9
  economic: 0.85
  technological: 0.95
  diplomatic: 0.8
  cyber: 0.9
"#,
                ),
                (
                    "organizations/nato.yaml",
                    r#"
id: nato
name: NATO
tier: organization
capabilities:
  military: 0.0
  economic: 0.6
  technological: 0.7
  diplomatic: 0.85
  cyber: 0.65
"#,
                ),
            ],
        );

        let registry = ProfileRegistry::load_from_dir(&base).unwrap();
        let nations = registry.of_tier(ActorTier::Nation);
        assert_eq!(nations.len(), 2);

        let orgs = registry.of_tier(ActorTier::Organization);
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].id, "nato");

        let corps = registry.of_tier(ActorTier::Corporation);
        assert!(corps.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_directory_returns_error() {
        let result = ProfileRegistry::load_from_dir(Path::new("/nonexistent/path/profiles"));
        assert!(result.is_err());
        if let Err(TianJiError::Input(msg)) = result {
            assert!(msg.contains("does not exist"));
        } else {
            panic!("Expected TianJiError::Input for missing directory");
        }
    }

    #[test]
    fn invalid_yaml_returns_error() {
        let base = tempfile_path("invalid_yaml");
        let _ = std::fs::remove_dir_all(&base);
        write_temp_profiles(
            &base,
            &[("nations/bad.yaml", "this is not: valid: yaml: [[[")],
        );

        let result = ProfileRegistry::load_from_dir(&base);
        assert!(result.is_err());
        match result.unwrap_err() {
            TianJiError::Yaml(_, path) => assert!(path.contains("bad.yaml")),
            other => panic!("Expected Yaml error, got: {other}"),
        }

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_directory_loads_zero_profiles() {
        let base = tempfile_path("empty_dir");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        let registry = ProfileRegistry::load_from_dir(&base).unwrap();
        assert!(registry.profiles.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn non_yaml_files_are_ignored() {
        let base = tempfile_path("non_yaml");
        let _ = std::fs::remove_dir_all(&base);
        write_temp_profiles(
            &base,
            &[
                (
                    "nations/china.yaml",
                    r#"
id: china
name: China
tier: nation
capabilities:
  military: 0.8
  economic: 0.7
  technological: 0.6
  diplomatic: 0.7
  cyber: 0.8
"#,
                ),
                ("nations/readme.md", "This is not a YAML file"),
                ("nations/data.json", "{\"not\": \"yaml\"}"),
            ],
        );

        let registry = ProfileRegistry::load_from_dir(&base).unwrap();
        assert_eq!(registry.profiles.len(), 1);
        assert!(registry.get("china").is_some());

        let _ = std::fs::remove_dir_all(&base);
    }

    fn tempfile_path(label: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::path::PathBuf::from(format!("/tmp/tianji_profile_test_{label}_{id}"))
    }
}
