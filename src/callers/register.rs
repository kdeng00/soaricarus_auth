use axum::{Json, http::StatusCode};

use crate::hashing;
use crate::repo;

pub mod request {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Deserialize, Serialize, utoipa::ToSchema)]
    pub struct Request {
        #[serde(skip_serializing_if = "String::is_empty")]
        pub username: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        pub password: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        pub email: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        pub phone: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        pub firstname: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        pub lastname: String,
    }
}

pub mod response {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, utoipa::ToSchema)]
    pub struct Response {
        pub message: String,
        pub data: Vec<icarus_models::user::User>,
    }
}

/// Endpoint to register a user
#[utoipa::path(
    post,
    path = super::endpoints::REGISTER,
    request_body(
        content = request::Request,
        description = "Data required to register",
        content_type = "application/json"
    ),
    responses(
        (status = 201, description = "User created", body = response::Response),
        (status = 404, description = "User already exists", body = response::Response),
        (status = 400, description = "Issue creating user", body = response::Response)
    )
)]
pub async fn register_user(
    axum::Extension(pool): axum::Extension<sqlx::PgPool>,
    Json(payload): Json<request::Request>,
) -> (StatusCode, Json<response::Response>) {
    let registration_enabled = match is_registration_enabled().await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Error: {err:?}");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(response::Response {
                    message: String::from("Registration check failed"),
                    data: Vec::new(),
                }),
            );
        }
    };

    if registration_enabled {
        let mut user = icarus_models::user::User {
            username: payload.username.clone(),
            password: payload.password.clone(),
            email: payload.email.clone(),
            phone: payload.phone.clone(),
            firstname: payload.firstname.clone(),
            lastname: payload.lastname.clone(),
            status: String::from("Active"),
            email_verified: true,
            ..Default::default()
        };

        match repo::user::exists(&pool, &user.username).await {
            Ok(res) => {
                if res {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(response::Response {
                            message: String::from("Error"),
                            data: Vec::new(),
                        }),
                    )
                } else {
                    let salt_string = hashing::generate_salt().unwrap();
                    let mut salt = icarus_models::user::salt::Salt::default();
                    let generated_salt = salt_string;
                    salt.salt = generated_salt.to_string();
                    salt.id = repo::salt::insert(&pool, &salt).await.unwrap();
                    user.salt_id = salt.id;
                    let hashed_password =
                        hashing::hash_password(&user.password, &generated_salt).unwrap();
                    user.password = hashed_password;

                    match repo::user::insert(&pool, &user).await {
                        Ok((id, date_created)) => {
                            user.id = id;
                            user.date_created = date_created;
                            (
                                StatusCode::CREATED,
                                Json(response::Response {
                                    message: String::from("User created"),
                                    data: vec![user],
                                }),
                            )
                        }
                        Err(err) => (
                            StatusCode::BAD_REQUEST,
                            Json(response::Response {
                                message: err.to_string(),
                                data: vec![user],
                            }),
                        ),
                    }
                }
            }
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(response::Response {
                    message: err.to_string(),
                    data: vec![user],
                }),
            ),
        }
    } else {
        (
            axum::http::StatusCode::NOT_ACCEPTABLE,
            Json(response::Response {
                message: String::from("Registration is not enabled"),
                data: Vec::new(),
            }),
        )
    }
}

/// Checks to see if registration is enabled
async fn is_registration_enabled() -> Result<bool, std::io::Error> {
    let key = String::from("ENABLE_REGISTRATION");
    let var = icarus_envy::environment::get_env(&key);
    let parsed_value = var.value.to_uppercase();

    if parsed_value == "TRUE" {
        Ok(true)
    } else if parsed_value == "FALSE" {
        Ok(false)
    } else {
        Err(std::io::Error::other(
            "Could not determine value of ENABLE_REGISTRATION",
        ))
    }
}
