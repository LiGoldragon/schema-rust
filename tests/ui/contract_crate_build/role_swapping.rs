use protos::WireContractFamily;
use schema_rust::build::{
    ContractCrateBuild, CrateName, SchemaVersion, UpdateEnvironmentVariable,
};

fn main() {
    let _ = ContractCrateBuild::new(
        "contract-root",
        SchemaVersion::new("0.1.0"),
        UpdateEnvironmentVariable::new("UPDATE_CONTRACT_SCHEMA"),
        CrateName::new("contract"),
        WireContractFamily::SignalSpirit,
    );
}
