use dotenv::dotenv;
use std::env;
use std::path::PathBuf;

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p license_manager --bin generate -- <output_dir> <activation_key> [count] [length]"
    );
    eprintln!("  <output_dir>     Directory where files will be written");
    eprintln!("  <activation_key> 32-byte key (base64, hex, or plain 32 char string)");
    eprintln!("  [count]          Optional number of codes to generate (default 10000)");
    eprintln!("  [length]         Optional length of each code (default 16)");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().unwrap();

    let mut args = env::args().skip(1);
    let Some(dir) = args.next() else {
        print_usage();
        std::process::exit(1);
    };
    let key = args
        .next()
        .or_else(|| env::var("ACTIVATION_MASTER_KEY").ok())
        .unwrap();

    let count = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|err| format!("invalid count: {err}"))?
        .unwrap_or(10_000);

    let length = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()
        .map_err(|err| format!("invalid length: {err}"))?
        .unwrap_or(16);

    let output_dir = PathBuf::from(dir);
    let artefacts =
        license_manager::bootstrap_activation_storage(&output_dir, &key, count, length)?;

    println!(
        "Plain activation codes  : {}",
        artefacts.plaintext_codes_path.display()
    );
    println!(
        "Encrypted store (server): {}",
        artefacts.encrypted_store_path.display()
    );
    println!(
        "Encrypted codes (client): {}",
        artefacts.encrypted_codes_path.display()
    );
    Ok(())
}
