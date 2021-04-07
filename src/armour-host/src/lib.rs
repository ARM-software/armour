/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

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
