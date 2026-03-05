use rustok_storefront::router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Note: modules::init_modules() is not easily exported if it was private in lib.rs,
    // but I'll assume we handle module init in the shared library or backend.

    let app = router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3100").await?;
    println!("Storefront running on http://0.0.0.0:3100");
    axum::serve(listener, app).await?;
    Ok(())
}
