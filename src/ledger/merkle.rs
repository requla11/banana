use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyChainRecord {
    pub sequence_number: u64,
    pub artifact_name: String,
    pub artifact_hash: String,
    pub builder_id: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InTotoSubject {
    pub name: String,
    pub digest: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlsaProvenancePredicate {
    #[serde(rename = "builder")]
    pub builder: SlsaBuilder,
    #[serde(rename = "buildType")]
    pub build_type: String,
    #[serde(rename = "invocation")]
    pub invocation: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlsaBuilder {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InTotoStatement {
    #[serde(rename = "_type")]
    pub statement_type: String,
    #[serde(rename = "subject")]
    pub subject: Vec<InTotoSubject>,
    #[serde(rename = "predicateType")]
    pub predicate_type: String,
    #[serde(rename = "predicate")]
    pub predicate: SlsaProvenancePredicate,
}

impl SupplyChainRecord {
    pub fn to_slsa_v1_statement(&self) -> InTotoStatement {
        let mut digests = HashMap::new();
        digests.insert("blake3".to_string(), self.artifact_hash.clone());

        InTotoStatement {
            statement_type: "https://in-toto.io/Statement/v1".to_string(),
            subject: vec![InTotoSubject {
                name: self.artifact_name.clone(),
                digest: digests,
            }],
            predicate_type: "https://slsa.dev/provenance/v1".to_string(),
            predicate: SlsaProvenancePredicate {
                builder: SlsaBuilder {
                    id: self.builder_id.clone(),
                },
                build_type: "https://requla.org/banana/build/v1".to_string(),
                invocation: serde_json::json!({
                    "sequenceNumber": self.sequence_number,
                    "timestamp": self.timestamp.to_rfc3339()
                }),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: String,
    pub root_hash: String,
    pub audit_path: Vec<(String, bool)>,
}

pub struct MerkleTree {
    leaves: Vec<String>,
    levels: Vec<Vec<String>>,
}

impl MerkleTree {
    pub fn new(records: &[SupplyChainRecord]) -> Self {
        let leaves: Vec<String> = records
            .iter()
            .map(|r| {
                let serialized = serde_json::to_vec(r).unwrap_or_default();
                blake3::hash(&serialized).to_hex().to_string()
            })
            .collect();

        if leaves.is_empty() {
            let empty_root = blake3::hash(b"").to_hex().to_string();
            return Self {
                leaves: Vec::new(),
                levels: vec![vec![empty_root]],
            };
        }

        let mut levels = vec![leaves.clone()];
        let mut current = leaves.clone();

        while current.len() > 1 {
            let mut next = Vec::new();
            for chunk in current.chunks(2) {
                if chunk.len() == 2 {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(chunk[0].as_bytes());
                    hasher.update(chunk[1].as_bytes());
                    next.push(hasher.finalize().to_hex().to_string());
                } else {
                    next.push(chunk[0].clone());
                }
            }
            levels.push(next.clone());
            current = next;
        }

        Self { leaves, levels }
    }

    pub fn root_hash(&self) -> String {
        self.levels
            .last()
            .and_then(|lvl| lvl.first())
            .cloned()
            .unwrap_or_else(|| blake3::hash(b"").to_hex().to_string())
    }

    pub fn generate_proof(&self, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.leaves.len() {
            return None;
        }

        let leaf_hash = self.leaves[leaf_index].clone();
        let root_hash = self.root_hash();
        let mut audit_path = Vec::new();
        let mut current_idx = leaf_index;

        for level in &self.levels[0..self.levels.len().saturating_sub(1)] {
            let is_right = current_idx % 2 == 1;
            let sibling_idx = if is_right {
                current_idx - 1
            } else {
                current_idx + 1
            };

            if sibling_idx < level.len() {
                audit_path.push((level[sibling_idx].clone(), is_right));
            }
            current_idx /= 2;
        }

        Some(MerkleProof {
            leaf_hash,
            root_hash,
            audit_path,
        })
    }

    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let mut current = proof.leaf_hash.clone();

        for (sibling, is_right) in &proof.audit_path {
            let mut hasher = blake3::Hasher::new();
            if *is_right {
                hasher.update(sibling.as_bytes());
                hasher.update(current.as_bytes());
            } else {
                hasher.update(current.as_bytes());
                hasher.update(sibling.as_bytes());
            }
            current = hasher.finalize().to_hex().to_string();
        }

        current == proof.root_hash
    }
}

pub struct LedgerWitness {
    signing_key: SigningKey,
    records: Vec<SupplyChainRecord>,
}

impl LedgerWitness {
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        Self {
            signing_key,
            records: Vec::new(),
        }
    }

    pub fn append_record(
        &mut self,
        artifact_name: impl Into<String>,
        artifact_hash: impl Into<String>,
        builder_id: impl Into<String>,
    ) -> u64 {
        let seq = self.records.len() as u64;
        let record = SupplyChainRecord {
            sequence_number: seq,
            artifact_name: artifact_name.into(),
            artifact_hash: artifact_hash.into(),
            builder_id: builder_id.into(),
            timestamp: Utc::now(),
        };
        self.records.push(record);
        seq
    }

    pub fn sign_root(&self, tree: &MerkleTree) -> (String, VerifyingKey) {
        let root = tree.root_hash();
        let signature = self.signing_key.sign(root.as_bytes());
        (signature.to_string(), self.signing_key.verifying_key())
    }

    pub fn build_tree(&self) -> MerkleTree {
        MerkleTree::new(&self.records)
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn records(&self) -> &[SupplyChainRecord] {
        &self.records
    }

    pub fn persist_to_disk(&self, path: &Path) -> Result<(), anyhow::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        for record in &self.records {
            let line = serde_json::to_string(record)?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    pub fn load_from_disk(path: &Path) -> Result<Self, anyhow::Error> {
        let mut witness = Self::new();
        if !path.exists() {
            return Ok(witness);
        }

        let file = OpenOptions::new().read(true).open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                let record: SupplyChainRecord = serde_json::from_str(&line)?;
                witness.records.push(record);
            }
        }
        Ok(witness)
    }
}

impl Default for LedgerWitness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_merkle_tree_proof_and_verification() {
        let mut witness = LedgerWitness::new();
        witness.append_record("artifact-1", "blake3:hash1", "banana-builder-v1");
        witness.append_record("artifact-2", "blake3:hash2", "banana-builder-v1");
        witness.append_record("artifact-3", "blake3:hash3", "banana-builder-v1");
        witness.append_record("artifact-4", "blake3:hash4", "banana-builder-v1");

        let tree = witness.build_tree();
        assert_eq!(witness.record_count(), 4);

        let proof = tree.generate_proof(2).unwrap();
        assert!(MerkleTree::verify_proof(&proof));

        let mut tampered = proof.clone();
        tampered.root_hash = "tampered-root".to_string();
        assert!(!MerkleTree::verify_proof(&tampered));
    }

    #[test]
    fn test_slsa_statement_and_persistence() {
        let temp = tempdir().unwrap();
        let log_file = temp.path().join("ledger.jsonl");

        let mut witness = LedgerWitness::new();
        witness.append_record("fish-cli", "blake3:abc123", "github-actions");
        let rec = &witness.records()[0];
        let statement = rec.to_slsa_v1_statement();
        assert_eq!(statement.statement_type, "https://in-toto.io/Statement/v1");
        assert_eq!(statement.predicate_type, "https://slsa.dev/provenance/v1");

        witness.persist_to_disk(&log_file).unwrap();
        assert!(log_file.exists());

        let loaded = LedgerWitness::load_from_disk(&log_file).unwrap();
        assert_eq!(loaded.record_count(), 1);
        assert_eq!(loaded.records()[0].artifact_name, "fish-cli");
    }
}
