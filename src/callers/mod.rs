pub mod common;
pub mod login;
pub mod register;

pub mod endpoints {
    pub const ROOT: &str = "/";
    pub const REGISTER: &str = "/api/v2/register";
    pub const DBTEST: &str = "/api/v2/test/db";
    pub const LOGIN: &str = "/api/v2/login";
    pub const UPDATE_PASSWORD: &str = "/api/v2/user/password";
    pub const SERVICE_LOGIN: &str = "/api/v2/service/login";
    pub const REFRESH_TOKEN: &str = "/api/v2/token/refresh";
}
