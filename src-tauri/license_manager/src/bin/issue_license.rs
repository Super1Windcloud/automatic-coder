use dotenv::dotenv;
use std::env;

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p license_manager --bin issue_license -- <private_key> <machine_id> <license_id> [expires_in_days] [customer]"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let mut args = env::args().skip(1);
    let Some(private_key) = args.next() else {
        print_usage();
        std::process::exit(1);
    };
    let Some(machine_id) = args.next() else {
        print_usage();
        std::process::exit(1);
    };
    let Some(license_id) = args.next() else {
        print_usage();
        std::process::exit(1);
    };

    let expires_at = args
        .next()
        .map(|days| days.parse::<u64>())
        .transpose()?
        .map(|days| license_manager::now_unix_seconds() + days * 24 * 60 * 60);
    let customer = args.next();

    let claims = license_manager::new_license_claims(
        license_id,
        machine_id,
        customer,
        expires_at,
        vec!["base".to_string()],
    );
    let signed = license_manager::sign_license(&private_key, claims)?;

    println!("{signed}");
    Ok(())
}
