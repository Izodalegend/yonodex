use reqwest::multipart::{Form, Part};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IpfsAddResponse {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Hash")]
    pub hash: String, // The CID
    #[serde(rename = "Size")]
    pub size: String,
}

/// Uploads a string (e.g., JSON trade receipt) to the local IPFS node.
/// Returns the Content Identifier (CID) if successful.
#[tauri::command]
pub async fn upload_to_ipfs(content: String) -> Result<String, String> {
    let url = "http://127.0.0.1:5001/api/v0/add";

    // Create a multipart form with the content as a file
    let part = Part::text(content)
        .file_name("yonodex-record.json")
        .mime_str("application/json")
        .map_err(|e| format!("MIME error: {e}"))?;
    
    let form = Form::new().part("file", part);

    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to IPFS daemon: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("IPFS error: {}", res.status()));
    }

    // The IPFS API returns JSON lines. We just need the first one.
    let text = res.text().await.map_err(|e| format!("Read error: {e}"))?;
    
    // Parse the JSON line
    let lines: Vec<&str> = text.trim().lines().collect();
    if lines.is_empty() {
        return Err("Empty response from IPFS".to_string());
    }
    
    let parsed: IpfsAddResponse = serde_json::from_str(lines[0])
        .map_err(|e| format!("Parse error: {e}"))?;

    Ok(parsed.hash)
}