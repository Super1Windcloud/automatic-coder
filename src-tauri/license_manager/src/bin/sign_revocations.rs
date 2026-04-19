use dotenv::dotenv;
use serde::Deserialize;
use std::{env, fs};

#[derive(Deserialize)]
struct RevocationInput {
    version: Option<u64>,
    revoked: Vec<String>,
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p license_manager --bin sign_revocations -- <private_key> <input_json> [output_json]"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let mut args = env::args().skip(1);
    let Some(private_key) = args.next() else {
        print_usage();
        std::process::exit(1);
    };
    let Some(input_path) = args.next() else {
        print_usage();
        std::process::exit(1);
    };
    let output_path = args.next();

    let input: RevocationInput = serde_json::from_str(&fs::read_to_string(&input_path)?)?;
    let signed = license_manager::sign_revocation_list(
        &private_key,
        license_manager::new_revocation_list(input.revoked, input.version.unwrap_or(1)),
    )?;

    if let Some(path) = output_path {
        fs::write(path, signed)?;
    } else {
        println!("{signed}");
    }

    Ok(())
}
