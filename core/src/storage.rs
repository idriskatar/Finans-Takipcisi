use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

/// Ayarlar ve favoriler uygulamanın veri klasöründe tutulur.
/// WebView origin'i (rastgele port) değişse de veriler korunur.
#[derive(Clone, Debug)]
pub struct Storage {
    pub path: PathBuf,
}

fn default_data() -> Value {
    json!({
        "settings": { "theme": "system", "bist": 30, "tcmb": 60, "spread": 5 },
        "favorites": []
    })
}

impl Storage {
    pub fn new(dir: PathBuf) -> Self {
        Self { path: dir.join("user-data.json") }
    }

    pub async fn load(&self) -> Value {
        match fs::read_to_string(&self.path).await {
            Ok(text) => serde_json::from_str::<Value>(&text).unwrap_or_else(|_| default_data()),
            Err(_) => default_data(),
        }
    }

    pub async fn save(&self, value: &Value) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp = self.path.with_extension("tmp");
        let body = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
        fs::write(&tmp, body).await?;
        fs::rename(&tmp, &self.path).await
    }

    fn cache_dir(&self) -> PathBuf {
        self.path.parent().unwrap_or_else(|| std::path::Path::new(".")).join("cache")
    }

    fn cache_path(&self, name: &str) -> PathBuf {
        self.cache_dir().join(format!("{name}.json"))
    }

    /// Son başarılı veriyi damgasıyla birlikte diske yazar.
    pub async fn save_cache(&self, name: &str, data: &Value) -> std::io::Result<()> {
        fs::create_dir_all(self.cache_dir()).await?;
        let wrapper = json!({
            "at": chrono::Utc::now().to_rfc3339(),
            "data": data,
        });
        let path = self.cache_path(name);
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_string(&wrapper).unwrap_or_default()).await?;
        fs::rename(&tmp, path).await
    }

    /// Diskdeki son veriyi okur ({ at, data } biçiminde).
    pub async fn load_cache(&self, name: &str) -> Option<Value> {
        let text = fs::read_to_string(self.cache_path(name)).await.ok()?;
        serde_json::from_str::<Value>(&text).ok()
    }
}
