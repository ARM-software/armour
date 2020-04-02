pub mod commands;
pub mod instance;
pub mod master;
pub mod rest_api;

pub async fn control_plane<T: serde::Serialize>(
    client: &actix_web::client::Client,
    method: http::Method,
    path: &str,
    value: &T,
) -> Result<(), String> {
    let full_path = format!("http://localhost:8088/controlplane/{}", path);
    let req = if method == http::Method::POST {
        client.post(full_path)
    } else if method == http::Method::DELETE {
        client.delete(full_path)
    } else {
        return Err("".into());
    };
    match req.send_json(value).await {
        Ok(mut res) => {
            if res.status().is_success() {
                Ok(())
            } else {
                let body = res.body().await.unwrap_or_default();
                let body = std::str::from_utf8(body.as_ref()).unwrap_or_default();
                Err(body.to_string())
            }
        }
        Err(err) => Err(err.to_string()),
    }
}
