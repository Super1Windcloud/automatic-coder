use dotenv::dotenv;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let (private_key, public_key) = license_manager::generate_signing_keypair();

    println!("LICENSE_PRIVATE_KEY={private_key}");
    println!("LICENSE_PUBLIC_KEY={public_key}");

    Ok(())
}
