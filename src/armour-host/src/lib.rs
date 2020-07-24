pub mod commands;
pub mod host;
pub mod instance;
pub mod rest_api;

fn string_from_bytes(b: bytes::Bytes) -> String {
    std::str::from_utf8(b.as_ref())
        .unwrap_or_default()
        .to_string()
}

pub async fn control_plane<S: serde::Serialize>(
    client: actix_web::client::Client,
    url: &url::Url,
    method: http::Method,
    path: &str,
    value: &S,
) -> Result<String, String> {
    if let Some(host_str) = url.host_str() {
        let full_path = format!(
            "https://{}:{}/{}",
            host_str,
            url.port().unwrap_or(8088),
            path
        );
        let req = match method {
            http::Method::GET => client.get(full_path),
            http::Method::POST => client.post(full_path),
            http::Method::DELETE => client.delete(full_path),
            _ => return Err("bad method".into()),
        };
        match req.send_json(value).await {
            Ok(mut res) => {
                let body = string_from_bytes(
                    res.body()
                        .await
                        .map_err(|_| "failed to read body".to_string())?,
                );
                // log::debug!("{:?}", body);
                if res.status().is_success() {
                    Ok(body)
                } else {
                    Err(body)
                }
            }
            Err(err) => Err(format!("{}: {}", url, err)),
        }
    } else {
        Err(format!("bad control plane URL: {}", url))
    }
}

pub async fn control_plane_deserialize<S, D>(
    client: actix_web::client::Client,
    url: &url::Url,
    method: http::Method,
    path: &str,
    value: &S,
) -> Result<D, String>
where
    S: serde::Serialize,
    D: serde::de::DeserializeOwned,
{
    let res = control_plane(client, url, method, path, value).await?;
    serde_json::from_slice(res.as_ref()).map_err(|_| res)
}
