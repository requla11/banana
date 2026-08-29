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
    extra_layers: Vec<Vec<u8>>,
}

impl Default for OciBuilder {
    fn default() -> Self {
        Self {
            entrypoint: Vec::new(),
            cmd: Vec::new(),
            env: vec!["PATH=/usr/local/bin:/usr/bin:/bin".to_string()],
            working_dir: "/app".to_string(),
            extra_layers: Vec::new(),
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

    pub fn add_raw_layer(mut self, layer_tar_gz: Vec<u8>) -> Self {
        self.extra_layers.push(layer_tar_gz);
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
                self.append_dir_deterministic(&mut tar_builder, rootfs_dir, Path::new(""))?;
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

        let mut diff_ids = vec![diff_id];
        let mut layer_descriptors = vec![OciDescriptor {
            media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
            digest: layer_digest.clone(),
            size: layer_size,
        }];
        let mut total_size = layer_size;

        for extra in &self.extra_layers {
            let mut hasher = Sha256::new();
            hasher.update(extra);
            let d = format!("sha256:{:x}", hasher.finalize());
            let sz = extra.len() as u64;
            diff_ids.push(d.clone());
            layer_descriptors.push(OciDescriptor {
                media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
                digest: d,
                size: sz,
            });
            total_size += sz;
        }

        let config_obj = OciConfig {
            architecture: "amd64".to_string(),
            os: "linux".to_string(),
            rootfs: OciRootfsConfig {
                rootfs_type: "layers".to_string(),
                diff_ids,
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
        total_size += config_size;

        let manifest_obj = OciManifest {
            schema_version: 2,
            media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
            config: OciDescriptor {
                media_type: "application/vnd.oci.image.config.v1+json".to_string(),
                digest: config_digest.clone(),
                size: config_size,
            },
            layers: layer_descriptors.clone(),
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
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
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
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        out_tar.append_data(&mut header, "index.json", index_bytes.as_slice())?;

        out_tar.finish()?;

        Ok(OciImageResult {
            manifest_digest,
            config_digest,
            layer_digests: layer_descriptors.into_iter().map(|l| l.digest).collect(),
            total_size_bytes: total_size,
        })
    }

    fn append_dir_deterministic<W: Write>(
        &self,
        tar: &mut Builder<W>,
        src_dir: &Path,
        dest_prefix: &Path,
    ) -> Result<(), anyhow::Error> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(src_dir)? {
            let entry = entry?;
            entries.push(entry.path());
        }
        entries.sort();

        for path in entries {
            let file_name = path.file_name().unwrap_or_default();
            let dest_path = dest_prefix.join(file_name);
            let dest_str = dest_path.to_string_lossy().replace('\\', "/");

            if path.is_dir() {
                let mut header = Header::new_gnu();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(0o755);
                header.set_size(0);
                header.set_mtime(0);
                header.set_uid(0);
                header.set_gid(0);
                header.set_cksum();
                tar.append_data(&mut header, &dest_str, &[][..])?;
                self.append_dir_deterministic(tar, &path, &dest_path)?;
            } else {
                let content = std::fs::read(&path)?;
                let mut header = Header::new_gnu();
                header.set_entry_type(tar::EntryType::Regular);
                header.set_mode(0o755);
                header.set_size(content.len() as u64);
                header.set_mtime(0);
                header.set_uid(0);
                header.set_gid(0);
                header.set_cksum();
                tar.append_data(&mut header, &dest_str, content.as_slice())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_oci_builder_deterministic_hashes() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let app_dir = rootfs.join("app");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("main"), b"deterministic binary content").unwrap();

        let out1 = temp.path().join("image1.tar");
        let out2 = temp.path().join("image2.tar");

        let builder1 = OciBuilder::new()
            .entrypoint(vec!["/app/main".to_string()])
            .working_dir("/app");
        let res1 = builder1.build_from_rootfs(&rootfs, &out1).unwrap();

        let builder2 = OciBuilder::new()
            .entrypoint(vec!["/app/main".to_string()])
            .working_dir("/app");
        let res2 = builder2.build_from_rootfs(&rootfs, &out2).unwrap();

        assert_eq!(res1.manifest_digest, res2.manifest_digest);
        assert_eq!(res1.config_digest, res2.config_digest);
        assert_eq!(res1.layer_digests, res2.layer_digests);
    }
}
