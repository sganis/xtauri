use confy;
use dirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub server: String,
    pub user: String,
    pub port: u16,
    pub home_dir: String,

    #[serde(skip_serializing)]
    pub private_key: Option<String>,

    #[serde(skip_serializing)]
    pub password: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        let home = String::from(dirs::home_dir().unwrap().to_string_lossy());
        let pkey = String::from(
            std::path::Path::new(home.as_str())
                .join(".ssh")
                .join("id_rsa_pem")
                .to_string_lossy(),
        );
        Self {
            server: "localhost".into(),
            user: "support".into(),
            password: Some("".into()),
            port: 22,
            home_dir: home,
            private_key: Some(pkey),
        }
    }
}

pub fn read_settings() -> Result<Settings, String> {
    let settings: Settings = match confy::load("studio", None) {
        Err(e) => {
            println!("error reading settings: {:?}", e);
            return Ok(Settings {
                ..Default::default()
            });
        }
        Ok(settings) => settings,
    };
    println!("settings: {:?}", settings);
    Ok(settings)
}

pub fn write_settings(settings: Settings) -> Result<(), String> {
    println!("writing settings: {:?}", settings);
    let s = Settings {
        password: Some("".into()),
        ..settings
    };
    match confy::store("studio", None, &s) {
        Err(e) => {
            println!("error reading settings: {:?}", e);
            Err(e.to_string())
        }
        Ok(_) => Ok(()),
    }
}
