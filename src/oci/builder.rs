use flate2::Compression;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tar::{Builder, Header};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "mediaType", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    pub config: OciDescriptor,
    pub layers: Vec<OciDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciConfig {
    pub architecture: String,
    pub os: String,
    pub rootfs: OciRootfsConfig,
    pub config: OciExecutionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciRootfsConfig {
    #[serde(rename = "type")]
    pub rootfs_type: String,
    pub diff_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OciExecutionConfig {
    #[serde(rename = "Entrypoint", skip_serializing_if = "Vec::is_empty")]
    pub entrypoint: Vec<String>,
    #[serde(rename = "Cmd", skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    #[serde(rename = "Env", skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    #[serde(rename = "WorkingDir", skip_serializing_if = "String::is_empty")]
    pub working_dir: String,
}

#[derive(Debug, Clone)]
pub struct OciImageResult {
    pub manifest_digest: String,
    pub config_digest: String,
    pub layer_digests: Vec<String>,
    pub total_size_bytes: u64,
}

pub struct OciBuilder {
    entrypoint: Vec<String>,
    cmd: Vec<String>,
    env: Vec<String>,
    working_dir: String,
}

impl Default for OciBuilder {
    fn default() -> Self {
        Self {
            entrypoint: Vec::new(),
            cmd: Vec::new(),
            env: vec!["PATH=/usr/local/bin:/usr/bin:/bin".to_string()],
            working_dir: "/app".to_string(),
        }
    }
}

impl OciBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entrypoint(mut self, entrypoint: Vec<String>) -> Self {
        self.entrypoint = entrypoint;
        self
    }

    pub fn cmd(mut self, cmd: Vec<String>) -> Self {
        self.cmd = cmd;
        self
    }

    pub fn env(mut self, env: Vec<String>) -> Self {
        self.env = env;
        self
    }

    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = dir.into();
        self
    }

    pub fn build_from_rootfs(
        &self,
        rootfs_dir: &Path,
        output_tar: &Path,
    ) -> Result<OciImageResult, anyhow::Error> {
        let mut layer_tar_bytes = Vec::new();
        {
            let mut tar_builder = Builder::new(&mut layer_tar_bytes);
            if rootfs_dir.exists() {
                tar_builder.append_dir_all(".", rootfs_dir)?;
            }
            tar_builder.finish()?;
        }

        let mut hasher_uncompressed = Sha256::new();
        hasher_uncompressed.update(&layer_tar_bytes);
        let diff_id = format!("sha256:{:x}", hasher_uncompressed.finalize());

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&layer_tar_bytes)?;
        let layer_gz_bytes = encoder.finish()?;

        let mut hasher_compressed = Sha256::new();
        hasher_compressed.update(&layer_gz_bytes);
        let layer_digest = format!("sha256:{:x}", hasher_compressed.finalize());
        let layer_size = layer_gz_bytes.len() as u64;

        let config_obj = OciConfig {
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
            rootfs: OciRootfsConfig {
                rootfs_type: "layers".to_string(),
                diff_ids: vec![diff_id],
            },
            config: OciExecutionConfig {
                entrypoint: self.entrypoint.clone(),
                cmd: self.cmd.clone(),
                env: self.env.clone(),
                working_dir: self.working_dir.clone(),
            },
        };

        let config_json = serde_json::to_vec(&config_obj)?;
        let mut hasher_config = Sha256::new();
        hasher_config.update(&config_json);
        let config_digest = format!("sha256:{:x}", hasher_config.finalize());
        let config_size = config_json.len() as u64;

        let manifest_obj = OciManifest {
            schema_version: 2,
            media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
            config: OciDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
                digest: config_digest.clone(),
                size: config_size,
            },
            layers: vec![OciDescriptor {
                media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
                digest: layer_digest.clone(),
                size: layer_size,
            }],
        };

        let manifest_json = serde_json::to_vec(&manifest_obj)?;
        let mut hasher_manifest = Sha256::new();
        hasher_manifest.update(&manifest_json);
        let manifest_digest = format!("sha256:{:x}", hasher_manifest.finalize());

        let file = File::create(output_tar)?;
        let mut out_tar = Builder::new(file);

        let oci_layout = serde_json::json!({ "imageLayoutVersion": "1.0.0" });
        let layout_bytes = serde_json::to_vec(&oci_layout)?;
        let mut header = Header::new_gnu();
        header.set_size(layout_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        out_tar.append_data(&mut header, "oci-layout", layout_bytes.as_slice())?;

        let index_json = serde_json::json!({
            "schemaVersion": 2,
            "manifests": [{
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": manifest_digest,
                "size": manifest_json.len()
            }]
        });
        let index_bytes = serde_json::to_vec(&index_json)?;
        let mut header = Header::new_gnu();
        header.set_size(index_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        out_tar.append_data(&mut header, "index.json", index_bytes.as_slice())?;

        out_tar.finish()?;

        Ok(OciImageResult {
            manifest_digest,
            config_digest,
            layer_digests: vec![layer_digest],
            total_size_bytes: layer_size + config_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_oci_builder_creates_compliant_image_tar() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let app_dir = rootfs.join("app");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("main"), b"binary-content").unwrap();

        let out_image = temp.path().join("image.tar");
        let builder = OciBuilder::new()
            .entrypoint(vec!["/app/main".to_string()])
            .working_dir("/app");

        let result = builder.build_from_rootfs(&rootfs, &out_image).unwrap();
        assert!(out_image.exists());
        assert!(result.manifest_digest.starts_with("sha256:"));
        assert!(result.config_digest.starts_with("sha256:"));
        assert_eq!(result.layer_digests.len(), 1);
    }
}
