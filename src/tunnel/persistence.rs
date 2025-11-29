use anyhow::{Context, Result};
use iroh::SecretKey;

pub async fn get_or_create_secret_key(persist: bool) -> Result<SecretKey> {
    let path = std::path::Path::new(".tunnel_key");

    if persist && path.exists() {
        let bytes = tokio::fs::read(path)
            .await
            .context("Failed to read .tunnel_key")?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid key length in .tunnel_key"))?;
        let key = SecretKey::from(bytes);
        println!("🔑 Loaded persistent secret key");
        Ok(key)
    } else {
        let key = SecretKey::generate(&mut rand::rng());

        if persist {
            tokio::fs::write(path, key.to_bytes())
                .await
                .context("Failed to write .tunnel_key")?;
            println!("🔑 Generated and saved new secret key");
        } else {
            println!("🔑 Generated ephemeral secret key (not persisted)");
        }

        Ok(key)
    }
}

pub async fn save_remote_peer_id(peer_id: iroh::PublicKey) -> Result<()> {
    let path = std::path::Path::new(".tunnel_peer");
    tokio::fs::write(path, peer_id.as_bytes())
        .await
        .context("Failed to write .tunnel_peer")?;
    Ok(())
}

pub async fn load_remote_peer_id() -> Option<iroh::PublicKey> {
    let path = std::path::Path::new(".tunnel_peer");
    if path.exists() {
        if let Ok(bytes) = tokio::fs::read(path).await {
            if let Ok(bytes_array) = bytes.try_into() {
                match iroh::PublicKey::from_bytes(&bytes_array) {
                    Ok(peer_id) => {
                        println!("🔗 Loaded persisted peer ID: {}", peer_id);
                        return Some(peer_id);
                    }
                    Err(_) => return None,
                }
            }
        }
    }
    None
}

pub async fn clear_remote_peer_id() -> Result<()> {
    let path = std::path::Path::new(".tunnel_peer");
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .context("Failed to remove .tunnel_peer")?;
        println!("🗑️  Cleared persisted peer ID");
    }
    Ok(())
}
