pub mod rep;

#[derive(Debug)]
struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, r#"docker API error ({})"#, self.0)
    }
}

impl std::error::Error for Error {}

#[derive(serde::Deserialize)]
struct Message {
    message: String,
}

impl From<(&str, &[u8])> for Error {
    fn from(e: (&str, &[u8])) -> Self {
        if let Ok(m) = serde_json::from_slice::<Message>(e.1) {
            Error(m.message)
        } else {
            Error(e.0.to_string())
        }
    }
}

/// Docker API
pub struct Docker(hyper::Client<hyper_unix_connector::UnixClient, hyper::Body>);

impl Default for Docker {
    fn default() -> Self {
        Docker(hyper::Client::builder().build(hyper_unix_connector::UnixClient))
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl Docker {
    /// Create a new Docker API object.
    pub fn new() -> Self {
        Docker::default()
    }
    fn addr(path: &str) -> hyper::Uri {
        hyper_unix_connector::Uri::new("/var/run/docker.sock", path).into()
    }
    async fn process_body(
        path: &str,
        mut res: hyper::Response<hyper::Body>,
    ) -> Result<hyper::http::response::Response<bytes::BytesMut>> {
        use futures::StreamExt;
        let mut body = bytes::BytesMut::new();
        while let Some(chunk) = res.body_mut().next().await {
            body.extend_from_slice(&chunk?)
        }
        if res.status().is_success() {
            Ok(res.map(|_| body))
        } else {
            Err(Box::new(Error::from((path, body.as_ref()))))
        }
    }
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        use bytes::buf::ext::BufExt;
        let res = Docker::process_body(path, self.0.get(Docker::addr(path)).await?).await?;
        serde_json::from_reader(res.into_body().reader()).map_err(|e| e.into())
    }
    async fn post(&self, path: &str) -> Result<hyper::Response<bytes::BytesMut>> {
        let req = hyper::Request::post(Docker::addr(path)).body(hyper::Body::from(""))?;
        Docker::process_body(path, self.0.request(req).await?).await
    }
}

/// System operations
impl Docker {
    /// Get [system information](https://docs.docker.com/engine/api/v1.40/#operation/SystemInfo).
    pub async fn info(&self) -> Result<rep::Info> {
        self.get("/info").await
    }
    /// Get [version](https://docs.docker.com/engine/api/v1.40/#operation/SystemVersion).
    pub async fn version(&self) -> Result<rep::Version> {
        self.get("/version").await
    }
}

/// Container operations
impl Docker {
    /// Get [list of containers](https://docs.docker.com/engine/api/v1.40/#operation/ContainerList).
    pub async fn containers(&self) -> Result<Vec<rep::Container>> {
        self.get("/containers/json?all=1").await
    }
    /// [Inspect a container](https://docs.docker.com/engine/api/v1.40/#operation/ContainerInspect).
    pub async fn inspect_container(&self, id: &str) -> Result<rep::ContainerDetails> {
        self.get(&format!("/containers/{}/json", id)).await
    }
    /// [Stop a container](https://docs.docker.com/engine/api/v1.40/#operation/ContainerStop).
    pub async fn stop_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/stop", id);
        self.post(&path).await.map(|_| ())
    }
    /// [Pause a container](https://docs.docker.com/engine/api/v1.40/#operation/ContainerPause).
    pub async fn pause_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/pause", id);
        self.post(&path).await.map(|_| ())
    }
    /// [Unpause a container](https://docs.docker.com/engine/api/v1.40/#operation/ContainerUnpause).
    pub async fn unpause_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/unpause", id);
        self.post(&path).await.map(|_| ())
    }
}

/// Image operations
impl Docker {
    /// Get [list of images](https://docs.docker.com/engine/api/v1.40/#operation/ImageList).
    pub async fn images(&self) -> Result<Vec<rep::Image>> {
        self.get("/images/json?all=1").await
    }
    /// [Inspect an image](https://docs.docker.com/engine/api/v1.40/#operation/ImageInspect).
    pub async fn inspect_image(&self, name: &str) -> Result<rep::ImageDetails> {
        self.get(&format!("/images/{}/json", name)).await
    }
    /// [Search for an image](https://docs.docker.com/engine/api/v1.40/#operation/ImageSearch) in Docker Hub.
    pub async fn search_images(
        &self,
        term: &str,
        official: Option<bool>,
        stars: usize,
    ) -> Result<Vec<rep::SearchResult>> {
        let filters = if let Some(official) = official {
            format!(
                r#"{{"is-official":["{}"],"stars":["{}"]}}"#,
                official, stars
            )
        } else {
            format!(r#"{{"stars":["{}"]}}"#, stars)
        };
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("term", term)
            .append_pair("filters", &filters)
            .finish();
        self.get(&format!("/images/search?{}", query)).await
    }
}

/// Network operations
impl Docker {
    /// Get [list of networks](https://docs.docker.com/engine/api/v1.40/#operation/NetworkList).
    pub async fn networks(&self) -> Result<Vec<rep::NetworkDetails>> {
        self.get("/networks").await
    }
    /// [Inspect a network](https://docs.docker.com/engine/api/v1.40/#operation/NetworkInspect).
    pub async fn inspect_network(&self, id: &str) -> Result<rep::NetworkDetails> {
        self.get(&format!("/networks/{}", id)).await
    }
}

/// Volume operations
impl Docker {
    /// Get [list of volumes](https://docs.docker.com/engine/api/v1.40/#operation/VolumeList).
    pub async fn volumes(&self) -> Result<rep::Volumes> {
        self.get("/volumes").await
    }
    /// [Inspect a volume](https://docs.docker.com/engine/api/v1.40/#operation/VolumeInspect).
    pub async fn inspect_volume(&self, name: &str) -> Result<rep::Volume> {
        self.get(&format!("/volumes/{}", name)).await
    }
}
