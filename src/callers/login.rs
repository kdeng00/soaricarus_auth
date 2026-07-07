pub mod request {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Deserialize, Serialize, utoipa::ToSchema)]
    pub struct Request {
        pub username: String,
        pub password: String,
    }

    pub mod service_login {
        #[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
        pub struct Request {
            pub passphrase: String,
        }
    }

    pub mod refresh_token {
        #[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
        pub struct Request {
            pub access_token: String,
        }
    }
}

pub mod response {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Deserialize, Serialize, utoipa::ToSchema)]
    pub struct Response {
        pub message: String,
        pub data: Vec<icarus_models::login_result::LoginResult>,
    }

    pub mod service_login {
        #[derive(Debug, Default, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
        pub struct Response {
            pub message: String,
            pub data: Vec<icarus_models::login_result::LoginResult>,
        }
    }

    pub mod refresh_token {
        #[derive(Debug, Default, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
        pub struct Response {
            pub message: String,
            pub data: Vec<icarus_models::login_result::LoginResult>,
        }
    }
}

/// Module for login endpoints
pub mod endpoint {
    use axum::{Json, http::StatusCode};

    use crate::hashing;
    use crate::repo;
    use crate::token_stuff;

    use super::request;
    use super::response;

    async fn not_found(message: &str) -> (StatusCode, Json<response::Response>) {
        (
            StatusCode::NOT_FOUND,
            Json(response::Response {
                message: String::from(message),
                data: Vec::new(),
            }),
        )
    }

    /// Endpoint to login
    #[utoipa::path(
        post,
        path = super::super::endpoints::LOGIN,
        request_body(
            content = request::Request,
            description = "Data required to login",
            content_type = "application/json"
        ),
        responses(
            (status = 200, description = "Successfully logged in", body = response::Response),
            (status = 404, description = "Could not login with credentials", body = response::Response)
        )
    )]
    pub async fn login(
        axum::Extension(pool): axum::Extension<sqlx::PgPool>,
        Json(payload): Json<request::Request>,
    ) -> (StatusCode, Json<response::Response>) {
        // Check if user exists
        match repo::user::get(&pool, &payload.username).await {
            Ok(user) => {
                if hashing::verify_password(&payload.password, user.password.clone()).unwrap() {
                    // Create token
                    let key = icarus_envy::environment::get_secret_key().value;
                    let (token_literal, duration) =
                        token_stuff::create_token(&key, &user.id).unwrap();

                    if token_stuff::verify_token(&key, &token_literal) {
                        let current_time = time::OffsetDateTime::now_utc();
                        let _ = repo::user::update_last_login(&pool, &user, &current_time).await;

                        (
                            StatusCode::OK,
                            Json(response::Response {
                                message: String::from("Successful"),
                                data: vec![icarus_models::login_result::LoginResult {
                                    id: user.id,
                                    username: user.username.clone(),
                                    token: token_literal,
                                    token_type: String::from(icarus_models::token::TOKEN_TYPE),
                                    expiration: duration,
                                }],
                            }),
                        )
                    } else {
                        return not_found("Could not verify token").await;
                    }
                } else {
                    return not_found("Error Hashing").await;
                }
            }
            Err(err) => {
                return not_found(&err.to_string()).await;
            }
        }
    }

    /// Endpoint to login as a service user
    #[utoipa::path(
        post,
        path = super::super::endpoints::SERVICE_LOGIN,
        request_body(
            content = request::service_login::Request,
            description = "Data required to login as a service user",
            content_type = "application/json"
        ),
        responses(
            (status = 200, description = "Login successful", body = response::Response),
            (status = 400, description = "Error logging in with credentials", body = response::Response)
        )
    )]
    pub async fn service_login(
        axum::Extension(pool): axum::Extension<sqlx::PgPool>,
        axum::Json(payload): axum::Json<request::service_login::Request>,
    ) -> (
        axum::http::StatusCode,
        axum::Json<response::service_login::Response>,
    ) {
        let mut response = response::service_login::Response::default();

        match repo::service::valid_passphrase(&pool, &payload.passphrase).await {
            Ok((id, username, _date_created)) => {
                let key = icarus_envy::environment::get_secret_key().value;
                let (token_literal, duration) =
                    token_stuff::create_service_token(&key, &id).unwrap();

                if token_stuff::verify_token(&key, &token_literal) {
                    let login_result = icarus_models::login_result::LoginResult {
                        id,
                        username,
                        token: token_literal,
                        token_type: String::from(icarus_models::token::TOKEN_TYPE),
                        expiration: duration,
                    };

                    response.data.push(login_result);
                    response.message = String::from("Successful");

                    (axum::http::StatusCode::OK, axum::Json(response))
                } else {
                    (axum::http::StatusCode::OK, axum::Json(response))
                }
            }
            Err(err) => {
                response.message = err.to_string();
                (axum::http::StatusCode::BAD_REQUEST, axum::Json(response))
            }
        }
    }

    /// Endpoint to retrieve a refresh token
    #[utoipa::path(
        post,
        path = super::super::endpoints::REFRESH_TOKEN,
        request_body(
            content = request::refresh_token::Request,
            description = "Data required to retrieve a refresh token",
            content_type = "application/json"
        ),
        responses(
            (status = 200, description = "Refresh token generated", body = response::Response),
            (status = 400, description = "Error verifying token", body = response::Response),
            (status = 404, description = "Could not validate token", body = response::Response),
            (status = 500, description = "Error extracting token", body = response::Response)
        )
    )]
    pub async fn refresh_token(
        axum::Extension(pool): axum::Extension<sqlx::PgPool>,
        axum::Json(payload): axum::Json<request::refresh_token::Request>,
    ) -> (
        axum::http::StatusCode,
        axum::Json<response::refresh_token::Response>,
    ) {
        let mut response = response::refresh_token::Response::default();
        let key = icarus_envy::environment::get_secret_key().value;

        if token_stuff::verify_token(&key, &payload.access_token) {
            let token_type = token_stuff::get_token_type(&key, &payload.access_token).unwrap();

            if token_stuff::is_token_type_valid(&token_type) {
                // Get passphrase record with id
                match token_stuff::extract_id_from_token(&key, &payload.access_token) {
                    Ok(id) => match repo::service::get_passphrase(&pool, &id).await {
                        Ok((username, _, _)) => {
                            match token_stuff::create_service_refresh_token(&key, &id) {
                                Ok((access_token, exp_dur)) => {
                                    let login_result = icarus_models::login_result::LoginResult {
                                        id,
                                        token: access_token,
                                        expiration: exp_dur,
                                        token_type: String::from(icarus_models::token::TOKEN_TYPE),
                                        username,
                                    };
                                    response.message = String::from("Successful");
                                    response.data.push(login_result);

                                    (axum::http::StatusCode::OK, axum::Json(response))
                                }
                                Err(err) => {
                                    response.message = err.to_string();
                                    (
                                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                        axum::Json(response),
                                    )
                                }
                            }
                        }
                        Err(err) => {
                            response.message = err.to_string();
                            (
                                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                axum::Json(response),
                            )
                        }
                    },
                    Err(err) => {
                        response.message = err.to_string();
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json(response),
                        )
                    }
                }
            } else {
                response.message = String::from("Invalid token type");
                (axum::http::StatusCode::NOT_FOUND, axum::Json(response))
            }
        } else {
            response.message = String::from("Could not verify token");
            (axum::http::StatusCode::BAD_REQUEST, axum::Json(response))
        }
    }
}
