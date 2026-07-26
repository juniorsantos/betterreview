pub struct Client {
    host: String,
}

impl Client {
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("https://{}/api/v3/{path}", self.host)
    }
}
