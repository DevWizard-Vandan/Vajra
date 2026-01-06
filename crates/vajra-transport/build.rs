//! Build script for vajra-transport.
//!
//! Compiles protobuf definitions and generates file descriptor set for reflection.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    // Configure tonic-build with file descriptor set for reflection
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("vajra_descriptor.bin"))
        .compile(
            &[
                "../../proto/vajra/v1/vector_service.proto",
                "../../proto/vajra/v1/raft_service.proto",
            ],
            &["../../proto"],
        )?;

    println!("cargo:rerun-if-changed=../../proto");

    Ok(())
}
