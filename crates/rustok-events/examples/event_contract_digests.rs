use std::path::PathBuf;

use rustok_events::event_contract_digests;
use serde::Serialize;

#[derive(Serialize)]
struct DigestArtifact {
    format_version: u16,
    registry: String,
    root_event: String,
    root_envelope: String,
    contract_payload: String,
    contract_envelope: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let digests = event_contract_digests();
    let artifact = DigestArtifact {
        format_version: 1,
        registry: digests.registry,
        root_event: digests.root_event,
        root_envelope: digests.root_envelope,
        contract_payload: digests.contract_payload,
        contract_envelope: digests.contract_envelope,
    };
    let json = format!("{}\n", serde_json::to_string_pretty(&artifact)?);

    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => print!("{json}"),
        [flag] if flag == "--write" => {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("contracts/event-contract-digests.json");
            std::fs::write(&path, json)?;
            println!("updated {}", path.display());
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "usage: cargo run -p rustok-events --example event_contract_digests [--write]",
            )
            .into());
        }
    }

    Ok(())
}
