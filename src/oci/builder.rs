use flate2::Compression;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tar::{Builder, Header};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciPlatform {
    pub architecture: String,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<OciPlatform>,
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

#[derive(Debug, Clone)]
pub struct OciMultiArchResult {
    pub index_digest: String,
    pub platform_manifests: Vec<(String, String)>,
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
            platform: None,
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
                platform: None,
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
                platform: None,
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

    pub fn build_multi_platform(
        &self,
        platforms: &[(&str, &str, &Path)],
        output_tar: &Path,
    ) -> Result<OciMultiArchResult, anyhow::Error> {
        let mut manifests = Vec::new();
        let mut platform_manifests = Vec::new();

        for (os, arch, rootfs) in platforms {
            let temp_tar = output_tar.with_extension(format!("{}-{}.tmp", os, arch));
            let res = self.build_from_rootfs(rootfs, &temp_tar)?;
            let _ = std::fs::remove_file(&temp_tar);

            manifests.push(serde_json::json!({
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": res.manifest_digest,
                "size": res.total_size_bytes,
                "platform": {
                    "architecture": arch,
                    "os": os
                }
            }));
            platform_manifests.push((format!("{}/{}", os, arch), res.manifest_digest));
        }

        let index_json = serde_json::json!({
            "schemaVersion": 2,
            "manifests": manifests
        });
        let index_bytes = serde_json::to_vec(&index_json)?;
        let mut hasher = Sha256::new();
        hasher.update(&index_bytes);
        let index_digest = format!("sha256:{:x}", hasher.finalize());

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

        let mut header_idx = Header::new_gnu();
        header_idx.set_size(index_bytes.len() as u64);
        header_idx.set_mode(0o644);
        header_idx.set_mtime(0);
        header_idx.set_uid(0);
        header_idx.set_gid(0);
        header_idx.set_cksum();
        out_tar.append_data(&mut header_idx, "index.json", index_bytes.as_slice())?;

        out_tar.finish()?;

        Ok(OciMultiArchResult {
            index_digest,
            platform_manifests,
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
    fn test_oci_multi_platform_fat_manifest() {
        let temp = tempdir().unwrap();
        let rootfs_amd64 = temp.path().join("rootfs-amd64");
        fs::create_dir_all(&rootfs_amd64).unwrap();
        fs::write(rootfs_amd64.join("app-x86"), b"amd64-binary").unwrap();

        let rootfs_arm64 = temp.path().join("rootfs-arm64");
        fs::create_dir_all(&rootfs_arm64).unwrap();
        fs::write(rootfs_arm64.join("app-arm"), b"arm64-binary").unwrap();

        let out_fat = temp.path().join("fat-image.tar");
        let builder = OciBuilder::new().working_dir("/app");
        let platforms = vec![
            ("linux", "amd64", rootfs_amd64.as_path()),
            ("linux", "arm64", rootfs_arm64.as_path()),
        ];
        let res = builder.build_multi_platform(&platforms, &out_fat).unwrap();
        assert!(out_fat.exists());
        assert_eq!(res.platform_manifests.len(), 2);
    }
}
