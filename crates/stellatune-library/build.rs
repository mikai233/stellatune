fn main() {
    // Use SQLx offline mode by default so `cargo build` works without requiring DATABASE_URL.
    // Query macros will read from the repo's `sqlx-data.json`.
    //
    // When DATABASE_URL is set (e.g. `cargo sqlx prepare`), allow online mode so SQL can be
    // validated against the live database.
    if std::env::var_os("DATABASE_URL").is_none() && std::env::var_os("SQLX_OFFLINE").is_none() {
        println!("cargo:rustc-env=SQLX_OFFLINE=true");
    }
}
