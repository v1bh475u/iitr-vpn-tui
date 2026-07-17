use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub const DEFAULT_GATEWAY: &str = "https://vpn.iitr.ac.in";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub gateway: String,
    pub username: String,
    pub auth_group: String,
    pub interface: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateway: DEFAULT_GATEWAY.to_owned(),
            username: String::new(),
            auth_group: "IITR-RA-VPN".to_owned(),
            interface: "iitr-vpn0".to_owned(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        if let Some(base) = env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(base).join("iitr-vpn/config.toml")
        } else if let Some(home) = env::var_os("HOME") {
            PathBuf::from(home).join(".config/iitr-vpn/config.toml")
        } else {
            PathBuf::from("iitr-vpn.toml")
        }
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents)
                .map_err(|error| format!("could not parse {}: {error}", path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("could not read {}: {error}", path.display())),
        }
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Self::path();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not secure {}: {error}", parent.display()))?;

        let encoded = toml::to_string_pretty(self)
            .map_err(|error| format!("could not encode configuration: {error}"))?;
        let temporary = path.with_extension("toml.tmp");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        file.write_all(encoded.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not secure {}: {error}", path.display()))?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_iitr_gateway() {
        let config = Config::default();
        assert_eq!(config.gateway, "https://vpn.iitr.ac.in");
        assert_eq!(config.auth_group, "IITR-RA-VPN");
        assert_eq!(config.interface, "iitr-vpn0");
    }

    #[test]
    fn secrets_are_not_part_of_the_config_schema() {
        let encoded = toml::to_string(&Config::default()).unwrap();
        assert!(!encoded.contains("password"));
        assert!(!encoded.contains("otp"));
    }
}
