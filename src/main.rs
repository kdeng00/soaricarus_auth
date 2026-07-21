pub mod callers;
pub mod config;
pub mod db;
pub mod hashing;
pub mod repo;
pub mod token_stuff;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let app = init::app().await;

    // run our app with hyper, listening globally on port 8001
    let url = config::get_full();
    let listener = tokio::net::TcpListener::bind(url).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

mod init {
    use axum::{
        Router,
        routing::{get, post},
    };
    use utoipa::OpenApi;

    use super::callers;
    use callers::common as common_callers;
    use callers::login as login_caller;
    use callers::register as register_caller;
    use login_caller::endpoint as login_endpoints;
    use login_caller::response as login_responses;
    use register_caller::response as register_responses;

    #[derive(utoipa::OpenApi)]
    #[openapi(
        paths(
            common_callers::endpoint::db_ping, common_callers::endpoint::root,
            register_caller::register_user,
            login_endpoints::login, login_endpoints::service_login, login_endpoints::refresh_token
            ),
        components(schemas(common_callers::response::TestResult,
                register_responses::Response,
            login_responses::Response, login_responses::service_login::Response, login_responses::refresh_token::Response)),
        tags(
            (name = "soaricarus Auth API", description = "Auth API for soaricarus API")
            )
    )]
    struct ApiDoc;

    mod cors {
        pub async fn configure_cors() -> tower_http::cors::CorsLayer {
            // Start building the CORS layer with common settings
            let cors = tower_http::cors::CorsLayer::new()
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                ]) // Specify allowed methods:cite[2]
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                ]) // Specify allowed headers:cite[2]
                .allow_credentials(true) // If you need to send cookies or authentication headers:cite[2]
                .max_age(std::time::Duration::from_secs(3600)); // Cache the preflight response for 1 hour:cite[2]

            // Dynamically set the allowed origin based on the environment
            match std::env::var(sienvy::keys::APP_ENV).as_deref() {
                Ok("production") => {
                    let allowed_origins_env = sienvy::environment::get_allowed_origins();
                    match sienvy::utility::delimitize(&allowed_origins_env) {
                        Ok(alwd) => {
                            let allowed_origins: Vec<axum::http::HeaderValue> = alwd
                                .into_iter()
                                .map(|s| s.parse::<axum::http::HeaderValue>().unwrap())
                                .collect();
                            cors.allow_origin(allowed_origins)
                        }
                        Err(err) => {
                            eprintln!(
                                "Could not parse out allowed origins from env: Error: {err:?}"
                            );
                            std::process::exit(-1);
                        }
                    }
                }
                _ => {
                    // Development (default): Allow localhost origins
                    cors.allow_origin(vec![
                        "http://localhost:4200".parse().unwrap(),
                        "http://127.0.0.1:4200".parse().unwrap(),
                    ])
                }
            }
        }
    }

    pub async fn routes() -> Router {
        // build our application with a route
        Router::new()
            .route(
                callers::endpoints::DBTEST,
                get(callers::common::endpoint::db_ping),
            )
            .route(
                callers::endpoints::ROOT,
                get(callers::common::endpoint::root),
            )
            .route(
                callers::endpoints::REGISTER,
                post(callers::register::register_user),
            )
            .route(
                callers::endpoints::LOGIN,
                post(callers::login::endpoint::login),
            )
            .route(
                callers::endpoints::SERVICE_LOGIN,
                post(callers::login::endpoint::service_login),
            )
            .route(
                callers::endpoints::REFRESH_TOKEN,
                post(callers::login::endpoint::refresh_token),
            )
            .layer(cors::configure_cors().await)
    }

    pub async fn app() -> Router {
        let pool = super::db::init::create_pool()
            .await
            .expect("Failed to create pool");

        super::db::init::migrations(&pool).await;

        routes()
            .await
            .merge(
                utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
            .layer(axum::Extension(pool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt; // for `call`, `oneshot`, and `ready`

    mod db_mgr {
        use std::str::FromStr;

        pub const LIMIT: usize = 6;

        pub async fn get_pool() -> Result<sqlx::PgPool, sqlx::Error> {
            let tm_db_url = sienvy::environment::get_db_url().value;
            let tm_options = sqlx::postgres::PgConnectOptions::from_str(&tm_db_url).unwrap();
            sqlx::PgPool::connect_with(tm_options).await
        }

        pub async fn generate_db_name() -> String {
            let db_name = get_database_name().await.unwrap()
                + &"_"
                + &uuid::Uuid::new_v4().to_string()[..LIMIT];
            db_name
        }

        pub async fn connect_to_db(db_name: &str) -> Result<sqlx::PgPool, sqlx::Error> {
            let db_url = sienvy::environment::get_db_url().value;
            let options = sqlx::postgres::PgConnectOptions::from_str(&db_url)?.database(db_name);
            sqlx::PgPool::connect_with(options).await
        }

        pub async fn create_database(
            template_pool: &sqlx::PgPool,
            db_name: &str,
        ) -> Result<(), sqlx::Error> {
            let create_query = format!("CREATE DATABASE {}", db_name);
            match sqlx::query(sqlx::AssertSqlSafe(create_query))
                .execute(template_pool)
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        }

        // Function to drop a database
        pub async fn drop_database(
            template_pool: &sqlx::PgPool,
            db_name: &str,
        ) -> Result<(), sqlx::Error> {
            let drop_query = format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", db_name);
            sqlx::query(sqlx::AssertSqlSafe(drop_query))
                .execute(template_pool)
                .await?;
            Ok(())
        }

        pub async fn get_database_name() -> Result<String, Box<dyn std::error::Error>> {
            let database_url = sienvy::environment::get_db_url().value;

            let parsed_url = url::Url::parse(&database_url)?;
            if parsed_url.scheme() == "postgres" || parsed_url.scheme() == "postgresql" {
                match parsed_url
                    .path_segments()
                    .and_then(|segments| segments.last().map(|s| s.to_string()))
                {
                    Some(sss) => Ok(sss),
                    None => Err("Error parsing".into()),
                }
            } else {
                // Handle other database types if needed
                Err("Error parsing".into())
            }
        }
    }

    fn get_test_register_request() -> callers::register::request::Request {
        callers::register::request::Request {
            username: String::from("somethingsss"),
            password: String::from("Raindown!"),
            email: String::from("dev@null.com"),
            phone: String::from("1234567890"),
            firstname: String::from("Bob"),
            lastname: String::from("Smith"),
        }
    }

    fn get_test_register_payload(usr: &callers::register::request::Request) -> serde_json::Value {
        json!({
            "username": &usr.username,
            "password": &usr.password,
            "email": &usr.email,
            "phone": &usr.phone,
            "firstname": &usr.firstname,
            "lastname": &usr.lastname,
        })
    }

    pub mod requests {
        use tower::ServiceExt; // for `call`, `oneshot`, and `ready`

        pub async fn register(
            app: &axum::Router,
            usr: &super::callers::register::request::Request,
        ) -> Result<axum::response::Response, std::convert::Infallible> {
            let payload = super::get_test_register_payload(&usr);
            let req = axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri(crate::callers::endpoints::REGISTER)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap();

            app.clone().oneshot(req).await
        }
    }

    #[tokio::test]
    async fn test_hello_world() {
        let app = init::app().await;

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(
                Request::builder()
                    .uri(callers::endpoints::ROOT)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Hello, World!");
    }

    #[tokio::test]
    async fn test_register_user() {
        let tm_pool = db_mgr::get_pool().await.unwrap();

        let db_name = db_mgr::generate_db_name().await;

        match db_mgr::create_database(&tm_pool, &db_name).await {
            Ok(_) => {
                println!("Success");
            }
            Err(e) => {
                assert!(false, "Error: {:?}", e.to_string());
            }
        }

        let pool = db_mgr::connect_to_db(&db_name).await.unwrap();

        db::init::migrations(&pool).await;

        let app = init::routes().await.layer(axum::Extension(pool));

        let usr = get_test_register_request();

        let response = requests::register(&app, &usr).await;

        match response {
            Ok(resp) => {
                assert_eq!(
                    resp.status(),
                    StatusCode::CREATED,
                    "Message: {:?} {:?}",
                    resp,
                    usr.username
                );
                let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .unwrap();
                let parsed_body: callers::register::response::Response =
                    serde_json::from_slice(&body).unwrap();
                let returned_usr = &parsed_body.data[0];

                assert_eq!(false, returned_usr.id.is_nil(), "Id is not populated");

                assert_eq!(
                    usr.username, returned_usr.username,
                    "Usernames do not match"
                );
                assert!(returned_usr.date_created.is_some(), "Date Created is empty");
            }
            Err(err) => {
                assert!(false, "Error: {:?}", err.to_string());
            }
        };

        let _ = db_mgr::drop_database(&tm_pool, &db_name).await;
    }

    #[tokio::test]
    async fn test_login_user() {
        let tm_pool = db_mgr::get_pool().await.unwrap();

        let db_name = db_mgr::generate_db_name().await;

        match db_mgr::create_database(&tm_pool, &db_name).await {
            Ok(_) => {
                println!("Success");
            }
            Err(e) => {
                assert!(false, "Error: {:?}", e.to_string());
            }
        }

        let pool = db_mgr::connect_to_db(&db_name).await.unwrap();

        db::init::migrations(&pool).await;

        let app = init::routes().await.layer(axum::Extension(pool));

        let usr = get_test_register_request();

        let response = requests::register(&app, &usr).await;

        match response {
            Ok(resp) => {
                assert_eq!(
                    resp.status(),
                    StatusCode::CREATED,
                    "Message: {:?} {:?}",
                    resp,
                    usr.username
                );
                let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                    .await
                    .unwrap();
                let parsed_body: callers::register::response::Response =
                    serde_json::from_slice(&body).unwrap();
                let returned_usr = &parsed_body.data[0];

                assert_eq!(false, returned_usr.id.is_nil(), "Id is not populated");

                assert_eq!(
                    usr.username, returned_usr.username,
                    "Usernames do not match"
                );
                assert!(returned_usr.date_created.is_some(), "Date Created is empty");

                let login_payload = json!({
                    "username": &usr.username,
                    "password": &usr.password,
                });

                match app
                    .oneshot(
                        Request::builder()
                            .method(axum::http::Method::POST)
                            .uri(callers::endpoints::LOGIN)
                            .header(axum::http::header::CONTENT_TYPE, "application/json")
                            .body(Body::from(login_payload.to_string()))
                            .unwrap(),
                    )
                    .await
                {
                    Ok(resp) => {
                        assert_eq!(StatusCode::OK, resp.status(), "Status is not right");
                        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                            .await
                            .unwrap();
                        let parsed_body: callers::login::response::Response =
                            serde_json::from_slice(&body).unwrap();
                        let login_result = &parsed_body.data[0];
                        assert!(!login_result.id.is_nil(), "Id is nil");
                    }
                    Err(err) => {
                        assert!(false, "Error: {:?}", err.to_string());
                    }
                }
            }
            Err(err) => {
                assert!(false, "Error: {:?}", err.to_string());
            }
        };

        let _ = db_mgr::drop_database(&tm_pool, &db_name).await;
    }

    #[tokio::test]
    async fn test_service_login_user() {
        let tm_pool = db_mgr::get_pool().await.unwrap();

        let db_name = db_mgr::generate_db_name().await;

        match db_mgr::create_database(&tm_pool, &db_name).await {
            Ok(_) => {
                println!("Success");
            }
            Err(e) => {
                assert!(false, "Error: {:?}", e.to_string());
            }
        }

        let pool = db_mgr::connect_to_db(&db_name).await.unwrap();

        db::init::migrations(&pool).await;

        let app = init::routes().await.layer(axum::Extension(pool));
        let passphrase =
            String::from("iUOo1fxshf3y1tUGn1yU8l9raPApHCdinW0VdCHdRFEjqhR3Bf02aZzsKbLtaDFH");
        let payload = serde_json::json!({
            "passphrase": passphrase
        });

        match app
            .oneshot(
                Request::builder()
                    .method(axum::http::Method::POST)
                    .uri(callers::endpoints::SERVICE_LOGIN)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
        {
            Ok(response) => {
                assert_eq!(StatusCode::OK, response.status(), "Status is not right");
                let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap();
                let parsed_body: callers::login::response::service_login::Response =
                    serde_json::from_slice(&body).unwrap();
                let _login_result = &parsed_body.data[0];
            }
            Err(err) => {
                assert!(false, "Error: {err:?}");
            }
        }

        let _ = db_mgr::drop_database(&tm_pool, &db_name).await;
    }

    #[tokio::test]
    async fn test_refresh_token() {
        let tm_pool = db_mgr::get_pool().await.unwrap();

        let db_name = db_mgr::generate_db_name().await;

        match db_mgr::create_database(&tm_pool, &db_name).await {
            Ok(_) => {
                println!("Success");
            }
            Err(e) => {
                assert!(false, "Error: {:?}", e.to_string());
            }
        }

        let pool = db_mgr::connect_to_db(&db_name).await.unwrap();

        db::init::migrations(&pool).await;

        let app = init::routes().await.layer(axum::Extension(pool));
        let id = uuid::Uuid::parse_str("22f9c775-cce9-457a-a147-9dafbb801f61").unwrap();
        let key = sienvy::environment::get_secret_key().value;

        match token_stuff::create_service_token(&key, &id) {
            Ok((token, _expire)) => {
                let payload = serde_json::json!({
                    "access_token": token
                });

                match app
                    .oneshot(
                        Request::builder()
                            .method(axum::http::Method::POST)
                            .uri(callers::endpoints::REFRESH_TOKEN)
                            .header(axum::http::header::CONTENT_TYPE, "application/json")
                            .body(Body::from(payload.to_string()))
                            .unwrap(),
                    )
                    .await
                {
                    Ok(response) => {
                        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                            .await
                            .unwrap();
                        let parsed_body: callers::login::response::service_login::Response =
                            serde_json::from_slice(&body).unwrap();
                        let login_result = &parsed_body.data[0];

                        assert_eq!(
                            id, login_result.id,
                            "The Id from the response does not match {id:?} {:?}",
                            login_result.id
                        );
                    }
                    Err(err) => {
                        assert!(false, "Error: {err:?}");
                    }
                }
            }
            Err(err) => {
                assert!(false, "Error: {err:?}");
            }
        }

        let _ = db_mgr::drop_database(&tm_pool, &db_name).await;
    }
}
