use argon2::{Argon2, PasswordVerifier, password_hash::PasswordHasher};
use rand::distr::SampleString;

pub struct SaltString {
    pub raw: String,
    pub in_bytes: Vec<u8>,
}

impl SaltString {
    pub fn generate() -> Result<SaltString, argon2::Error> {
        let mut rng = rand::rng();
        let s = rand::distr::Alphanumeric.sample_string(&mut rng, 20);
        let salt_string = SaltString {
            raw: s.to_string(),
            in_bytes: s.into_bytes(),
        };

        Ok(salt_string)
    }

    pub fn from_b64(s: &str) -> Result<SaltString, argon2::password_hash::Error> {
        let salt_string = SaltString {
            raw: (&s).to_string(),
            in_bytes: s.to_string().into_bytes(),
        };

        Ok(salt_string)
    }

    pub fn to_string(&self) -> &String {
        &self.raw
    }
}

pub fn generate_salt() -> Result<SaltString, argon2::Error> {
    SaltString::generate()
}

pub fn get_salt(s: &str) -> Result<SaltString, argon2::password_hash::Error> {
    SaltString::from_b64(s)
}

pub fn hash_password(
    password: &String,
    salt: &SaltString,
) -> Result<String, argon2::password_hash::Error> {
    let password_bytes = password.as_bytes();

    let argon2 = Argon2::default();

    Ok(argon2
        .hash_password_with_salt(password_bytes, &salt.in_bytes)?
        .to_string())
}

pub fn verify_password(
    password_attempt: &String,
    stored_hash: String,
) -> Result<bool, argon2::password_hash::Error> {
    let password_bytes = password_attempt.as_bytes();

    // Parse the stored hash string
    // This extracts the salt, parameters, and hash digest
    let parsed_hash = argon2::PasswordHash::new(stored_hash.as_str())?;

    // Create an Argon2 instance (it will use the parameters from the parsed hash)
    // Verify the password against the parsed hash
    // This automatically uses the correct salt and parameters embedded in `parsed_hash`
    match Argon2::default().verify_password(password_bytes, &parsed_hash) {
        Ok(()) => Ok(true), // Passwords match
        Err(argon2::password_hash::Error::PasswordInvalid) => Ok(false), // Passwords don't match
        Err(e) => Err(e),   // Some other error occurred (e.g., invalid hash format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const TEST_PASSWORD: &str = "somethingrandom";
    pub const TEST_WRONG_PASSWORD: &str = "Differentanotherlevel";

    #[test]
    fn test_hash_password() {
        let some_password = String::from(TEST_PASSWORD);
        let salt = generate_salt().unwrap();

        match hash_password(&some_password, &salt) {
            Ok(p) => match verify_password(&some_password, p.clone()) {
                Ok(res) => {
                    assert_eq!(res, true);
                }
                Err(err) => {
                    assert!(false, "Error: {:?}", err.to_string());
                }
            },
            Err(eerr) => {
                assert!(false, "Error: {:?}", eerr.to_string());
            }
        }
    }

    #[test]
    fn test_wrong_password() {
        let some_password = String::from(TEST_PASSWORD);
        let salt = generate_salt().unwrap();

        match hash_password(&some_password, &salt) {
            Ok(p) => {
                match verify_password(&some_password, p.clone()) {
                    Ok(res) => {
                        assert_eq!(res, true, "Passwords are not verified");
                    }
                    Err(err) => {
                        assert!(false, "Error: {:?}", err.to_string());
                    }
                }
                let wrong_password = String::from(TEST_WRONG_PASSWORD);
                let result = verify_password(&wrong_password, p.clone()).unwrap();
                assert_eq!(false, result, "Passwords should not match");
            }
            Err(err) => {
                assert!(false, "Error: {:?}", err.to_string());
            }
        }
    }
}
