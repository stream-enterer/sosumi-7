mod sim;

use std::path::PathBuf;

use sim::EconBridge;
use world::{EconState, TaxRates};

fn main() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("egopol is inside workspace")
        .to_path_buf();

    let sysimage_path = workspace_root.join("julia/sysimage/egopol_sysimage.so");
    let sysimage = if sysimage_path.exists() {
        println!("Using sysimage: {}", sysimage_path.display());
        Some(sysimage_path.as_path())
    } else {
        println!("No sysimage found, using default (slow startup).");
        None
    };

    println!("Initializing Julia runtime...");
    let mut bridge = EconBridge::new(&workspace_root, sysimage).expect("Failed to init Julia");
    println!("Julia runtime ready.");

    let rates = TaxRates::default();
    let idx = bridge
        .init_country("AT", &rates)
        .expect("Failed to init Austria model");
    println!(
        "Initialized Austria model (index={}, total={})",
        idx,
        bridge.model_count()
    );

    let mut econ_state = EconState::default();

    for q in 0..4 {
        let mut snapshot = bridge.step(idx).expect("Failed to step Austria model");
        snapshot.quarter = q;
        println!(
            "Q{}: GDP={:.2}  inflation={:.4}  unemployment={:.4}  euribor={:.4}",
            q, snapshot.real_gdp, snapshot.inflation, snapshot.unemployment, snapshot.euribor
        );
        econ_state.history.push(snapshot);
    }

    println!(
        "\nSimulation complete. {} snapshots recorded.",
        econ_state.history.len()
    );
}
