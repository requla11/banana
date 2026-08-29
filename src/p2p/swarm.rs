use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactChunk {
    pub index: u32,
    pub offset: u64,
    pub size: usize,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkManifest {
    pub artifact_hash: String,
    pub total_bytes: u64,
    pub chunk_size: usize,
    pub chunks: Vec<ArtifactChunk>,
}

impl ChunkManifest {
    pub fn create_from_bytes(artifact_hash: &str, data: &[u8], chunk_size: usize) -> Self {
        let chunk_size = if chunk_size == 0 {
            1024 * 1024
        } else {
            chunk_size
        };
        let mut chunks = Vec::new();
        let total_bytes = data.len() as u64;

        let mut offset = 0;
        let mut idx = 0;
        while offset < total_bytes {
            let end = (offset + chunk_size as u64).min(total_bytes) as usize;
            let slice = &data[offset as usize..end];
            let hash = blake3::hash(slice).to_hex().to_string();

            chunks.push(ArtifactChunk {
                index: idx,
                offset,
                size: slice.len(),
                checksum: hash,
            });

            offset += slice.len() as u64;
            idx += 1;
        }

        Self {
            artifact_hash: artifact_hash.to_string(),
            total_bytes,
            chunk_size,
            chunks,
        }
    }

    pub fn verify_chunk(&self, chunk_index: usize, chunk_data: &[u8]) -> bool {
        if let Some(chunk) = self.chunks.get(chunk_index) {
            if chunk.size != chunk_data.len() {
                return false;
            }
            let hash = blake3::hash(chunk_data).to_hex().to_string();
            chunk.checksum == hash
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerDescriptor {
    pub node_id: String,
    pub addr: SocketAddr,
    pub available_artifacts: Vec<String>,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UdpBeaconMessage {
    Announce {
        node_id: String,
        port: u16,
        artifacts: Vec<String>,
    },
    Query {
        artifact_hash: String,
    },
    QueryResponse {
        node_id: String,
        artifact_hash: String,
        has_artifact: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpFrameHeader {
    pub magic: u32,
    pub artifact_hash: String,
    pub chunk_index: u32,
    pub payload_size: u32,
}

#[derive(Clone)]
pub struct P2PNode {
    pub id: String,
    pub addr: SocketAddr,
    artifacts: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl P2PNode {
    pub fn new(id: impl Into<String>, addr: SocketAddr) -> Self {
        Self {
            id: id.into(),
            addr,
            artifacts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn store_artifact(&self, hash: &str, data: Vec<u8>) {
        let mut map = self.artifacts.write().unwrap();
        map.insert(hash.to_string(), data);
    }

    pub fn get_artifact(&self, hash: &str) -> Option<Vec<u8>> {
        let map = self.artifacts.read().unwrap();
        map.get(hash).cloned()
    }

    pub fn list_artifacts(&self) -> Vec<String> {
        let map = self.artifacts.read().unwrap();
        map.keys().cloned().collect()
    }

    pub fn create_beacon_payload(&self) -> Vec<u8> {
        let msg = UdpBeaconMessage::Announce {
            node_id: self.id.clone(),
            port: self.addr.port(),
            artifacts: self.list_artifacts(),
        };
        serde_json::to_vec(&msg).unwrap_or_default()
    }
}

pub struct P2PSwarmManager {
    local_node: P2PNode,
    peers: Arc<RwLock<HashMap<String, PeerDescriptor>>>,
}

impl P2PSwarmManager {
    pub fn new(local_node: P2PNode) -> Self {
        Self {
            local_node,
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_peer(&self, peer: PeerDescriptor) {
        let mut peers = self.peers.write().unwrap();
        peers.insert(peer.node_id.clone(), peer);
    }

    pub fn remove_peer(&self, node_id: &str) -> Option<PeerDescriptor> {
        let mut peers = self.peers.write().unwrap();
        peers.remove(node_id)
    }

    pub fn find_peers_with_artifact(&self, artifact_hash: &str) -> Vec<PeerDescriptor> {
        let peers = self.peers.read().unwrap();
        peers
            .values()
            .filter(|p| p.available_artifacts.iter().any(|a| a == artifact_hash))
            .cloned()
            .collect()
    }

    pub fn select_rarest_piece(
        &self,
        missing_chunks: &[u32],
        artifact_hash: &str,
    ) -> Option<(u32, Vec<PeerDescriptor>)> {
        if missing_chunks.is_empty() {
            return None;
        }

        let candidates = self.find_peers_with_artifact(artifact_hash);
        if candidates.is_empty() {
            return None;
        }

        let mut rarity_map: HashMap<u32, usize> = HashMap::new();
        for &chunk_idx in missing_chunks {
            rarity_map.insert(chunk_idx, candidates.len());
        }

        let rarest_idx = missing_chunks
            .iter()
            .min_by_key(|&&idx| rarity_map.get(&idx).copied().unwrap_or(usize::MAX))
            .copied()?;

        Some((rarest_idx, candidates))
    }

    pub fn sync_artifact_from_swarm(
        &self,
        artifact_hash: &str,
        _chunk_size: usize,
    ) -> Option<Vec<u8>> {
        if let Some(local_data) = self.local_node.get_artifact(artifact_hash) {
            return Some(local_data);
        }

        let candidates = self.find_peers_with_artifact(artifact_hash);
        if candidates.is_empty() {
            return None;
        }

        None
    }

    pub fn handle_beacon_packet(
        &self,
        sender_addr: SocketAddr,
        packet: &[u8],
    ) -> Option<PeerDescriptor> {
        if let Ok(UdpBeaconMessage::Announce {
            node_id,
            port,
            artifacts,
        }) = serde_json::from_slice::<UdpBeaconMessage>(packet)
            && node_id != self.local_node.id
        {
            let mut effective_addr = sender_addr;
            effective_addr.set_port(port);
            let peer = PeerDescriptor {
                node_id,
                addr: effective_addr,
                available_artifacts: artifacts,
                latency_ms: 1,
            };
            self.register_peer(peer.clone());
            return Some(peer);
        }
        None
    }

    pub async fn run_tcp_server(&self, listener: TcpListener) -> Result<(), anyhow::Error> {
        loop {
            let (mut socket, _) = listener.accept().await?;
            let local_node = self.local_node.clone();

            tokio::spawn(async move {
                let mut len_buf = [0u8; 4];
                if socket.read_exact(&mut len_buf).await.is_ok() {
                    let header_len = u32::from_be_bytes(len_buf) as usize;
                    let mut header_buf = vec![0u8; header_len];
                    if socket.read_exact(&mut header_buf).await.is_ok()
                        && let Ok(header) = serde_json::from_slice::<TcpFrameHeader>(&header_buf)
                        && let Some(data) = local_node.get_artifact(&header.artifact_hash)
                    {
                        let manifest = ChunkManifest::create_from_bytes(
                            &header.artifact_hash,
                            &data,
                            1024 * 1024,
                        );
                        if let Some(chunk) = manifest.chunks.get(header.chunk_index as usize) {
                            let start = chunk.offset as usize;
                            let end = (chunk.offset + chunk.size as u64) as usize;
                            let chunk_slice = &data[start..end];

                            let resp_len = (chunk_slice.len() as u32).to_be_bytes();
                            let _ = socket.write_all(&resp_len).await;
                            let _ = socket.write_all(chunk_slice).await;
                        }
                    }
                }
            });
        }
    }

    pub async fn fetch_chunk_over_tcp(
        peer_addr: SocketAddr,
        artifact_hash: &str,
        chunk_index: u32,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let mut stream = TcpStream::connect(peer_addr).await?;
        let header = TcpFrameHeader {
            magic: 0x42414E41,
            artifact_hash: artifact_hash.to_string(),
            chunk_index,
            payload_size: 0,
        };
        let header_bytes = serde_json::to_vec(&header)?;
        let len_bytes = (header_bytes.len() as u32).to_be_bytes();

        stream.write_all(&len_bytes).await?;
        stream.write_all(&header_bytes).await?;

        let mut resp_len_buf = [0u8; 4];
        stream.read_exact(&mut resp_len_buf).await?;
        let payload_len = u32::from_be_bytes(resp_len_buf) as usize;

        let mut payload = vec![0u8; payload_len];
        stream.read_exact(&mut payload).await?;

        Ok(payload)
    }

    pub async fn broadcast_beacon(
        socket: &UdpSocket,
        target_addr: SocketAddr,
        node: &P2PNode,
    ) -> Result<(), anyhow::Error> {
        let payload = node.create_beacon_payload();
        socket.send_to(&payload, target_addr).await?;
        Ok(())
    }

    pub fn get_local_node(&self) -> &P2PNode {
        &self.local_node
    }

    pub fn peer_count(&self) -> usize {
        let peers = self.peers.read().unwrap();
        peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_manifest_creation_and_verification() {
        let data = b"hello banana p2p swarm test payload";
        let hash = blake3::hash(data).to_hex().to_string();
        let manifest = ChunkManifest::create_from_bytes(&hash, data, 8);

        assert_eq!(manifest.artifact_hash, hash);
        assert!(!manifest.chunks.is_empty());

        let chunk0_data = &data[0..8];
        assert!(manifest.verify_chunk(0, chunk0_data));
        assert!(!manifest.verify_chunk(0, b"wrongdat"));
    }

    #[test]
    fn test_rarest_first_piece_selection() {
        let addr1: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let node1 = P2PNode::new("node-alpha", addr1);
        let manager = P2PSwarmManager::new(node1);

        let peer_addr: SocketAddr = "127.0.0.1:8081".parse().unwrap();
        let peer = PeerDescriptor {
            node_id: "node-beta".to_string(),
            addr: peer_addr,
            available_artifacts: vec!["hash-xyz".to_string()],
            latency_ms: 10,
        };
        manager.register_peer(peer);

        let missing = vec![0, 1, 2];
        let selected = manager.select_rarest_piece(&missing, "hash-xyz");
        assert!(selected.is_some());
        let (rarest_idx, candidates) = selected.unwrap();
        assert_eq!(rarest_idx, 0);
        assert_eq!(candidates.len(), 1);
    }
}
