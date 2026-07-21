use sqlx::postgres::PgPoolOptions;

pub async fn create_pool() -> Result<sqlx::PgPool, sqlx::Error> {
    let database_url = sienvy::environment::get_db_url().value;
    println!("Database url: {database_url}");

    PgPoolOptions::new()
        .max_connections(super::connection_settings::MAXCONN)
        .connect(&database_url)
        .await
}

pub async fn migrations(pool: &sqlx::PgPool) {
    // Run migrations using the sqlx::migrate! macro
    // Assumes your migrations are in a ./migrations folder relative to Cargo.toml
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("Failed to run migrations");
}
