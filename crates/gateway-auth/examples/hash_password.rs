//! Generate an Argon2id password hash for use with seed-admin.sh.
//!
//! Usage:
//!   cargo run -p gateway-auth --example hash_password -- "YourP@ssw0rd!"

use gateway_auth::password::{validate_password_strength, PasswordHasherService};

fn main() {
    let password = std::env::args()
        .nth(1)
        .expect("Usage: cargo run -p gateway-auth --example hash_password -- <password>");

    if let Err(e) = validate_password_strength(&password) {
        eprintln!("Password does not meet strength requirements: {e}");
        std::process::exit(1);
    }

    let hasher = PasswordHasherService::new();
    match hasher.hash_password(&password) {
        Ok(hash) => println!("{hash}"),
        Err(e) => {
            eprintln!("Failed to hash password: {e}");
            std::process::exit(1);
        }
    }
}
