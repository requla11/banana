use banana::{
    DependencyGraph, EnergyMeter, HardwareProfile, LedgerWitness, OciBuilder, P2PNode,
    P2PSwarmManager, PolyglotAstEngine,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "banana")]
#[command(author = "requla11")]
#[command(version = "0.2.0")]
#[command(
    about = "Production-grade P2P artifact mesh, zero-daemon OCI container builder, SLSA v1 ledger, RAPL energy meter, and polyglot AST engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    P2p {
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
        #[arg(long, default_value = "node-local")]
        node_id: String,
    },
    Oci {
        #[arg(long)]
        rootfs: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "/app")]
        working_dir: String,
    },
    Ledger {
        #[arg(long)]
        artifact: String,
        #[arg(long)]
        hash: String,
        #[arg(long, default_value = "banana-builder")]
        builder: String,
        #[arg(long)]
        persist: Option<PathBuf>,
    },
    Telemetry {
        #[arg(long, default_value_t = 65.0)]
        tdp_watts: f64,
        #[arg(long, default_value_t = 300.0)]
        grid_intensity: f64,
    },
    Ast {
        #[arg(long)]
        file: PathBuf,
    },
    CheckCycles {
        #[arg(long)]
        files: Vec<PathBuf>,
    },
    Info,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    match cli.command {
        Commands::P2p { addr, node_id } => {
            let socket_addr = addr.parse()?;
            let node = P2PNode::new(node_id, socket_addr);
            let manager = P2PSwarmManager::new(node);
            println!(
                "Banana P2P Swarm Manager initialized on {} (Peers: {})",
                addr,
                manager.peer_count()
            );
        }
        Commands::Oci {
            rootfs,
            output,
            working_dir,
        } => {
            let builder = OciBuilder::new().working_dir(working_dir);
            let res = builder.build_from_rootfs(&rootfs, &output)?;
            println!("OCI Image built successfully: {}", output.display());
            println!("Manifest Digest: {}", res.manifest_digest);
            println!("Config Digest:   {}", res.config_digest);
            println!("Total Size:      {} bytes", res.total_size_bytes);
        }
        Commands::Ledger {
            artifact,
            hash,
            builder,
            persist,
        } => {
            let mut witness = LedgerWitness::new();
            let seq = witness.append_record(&artifact, &hash, &builder);
            let tree = witness.build_tree();
            let (sig, _) = witness.sign_root(&tree);
            let stmt = witness.records()[0].to_slsa_v1_statement();
            println!("Supply Chain Record #{} appended", seq);
            println!("SLSA v1 Statement Type: {}", stmt.statement_type);
            println!("Merkle Root: {}", tree.root_hash());
            println!("Signature:   {}", sig);
            if let Some(path) = persist {
                witness.persist_to_disk(&path)?;
                println!("Persisted ledger to: {}", path.display());
            }
        }
        Commands::Telemetry {
            tdp_watts,
            grid_intensity,
        } => {
            let profile = HardwareProfile {
                tdp_watts,
                idle_power_watts: 10.0,
                core_count: std::thread::available_parallelism()
                    .map(|p| p.get())
                    .unwrap_or(4),
            };
            let mut meter = EnergyMeter::new(profile, grid_intensity);
            meter.start();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let metrics = meter.stop(0.5);
            println!(
                "Energy Joules: {:.4} J (Hardware RAPL: {})",
                metrics.estimated_joules, metrics.is_hardware_measured
            );
            println!("Carbon Impact: {:.6} gCO2", metrics.carbon_grams_co2);
        }
        Commands::Ast { file } => {
            if !file.exists() {
                anyhow::bail!("File not found: {}", file.display());
            }
            let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("");
            let content = std::fs::read_to_string(&file)?;
            let symbols = PolyglotAstEngine::extract_symbols(&content, ext);
            println!(
                "Extracted {} symbols from {}",
                symbols.len(),
                file.display()
            );
            for sym in symbols {
                println!(" - {:?}: {}", sym.kind, sym.name);
            }
        }
        Commands::CheckCycles { files } => {
            let mut graph = DependencyGraph::new();
            for f in files {
                graph.nodes.insert(f);
            }
            let cycles = graph.detect_cycles();
            if cycles.is_empty() {
                println!("No circular dependency cycles detected.");
            } else {
                println!("Detected {} cycle(s):", cycles.len());
                for c in cycles {
                    println!(" -> {:?}", c);
                }
            }
        }
        Commands::Info => {
            println!("Banana Universal Production Infrastructure Suite v0.2.0");
            println!(
                "Engines: Live UDP/TCP P2P Swarm, Deterministic OCI Builder, SLSA v1 Merkle Ledger, Kernel RAPL Telemetry, Polyglot AST Cycle Solver"
            );
        }
    }

    Ok(())
}
