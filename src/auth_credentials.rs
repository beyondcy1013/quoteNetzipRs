use std::env;

pub const DEFAULT_ACCOUNT: &str = "1522";
pub const ACCOUNT_ENV: &str = "NETZIP_TDX_ACCOUNT";
pub const PASSWORD_ENV: &str = "NETZIP_TDX_PASSWORD";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthCredentials {
    pub account: String,
    pub password: Option<String>,
    pub password_source: &'static str,
}

/// Loads credentials without printing or serializing the password.
pub fn load(account_fallback: Option<&str>, password_fallback: Option<&str>) -> AuthCredentials {
    let account = env::var(ACCOUNT_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| account_fallback.map(str::to_owned))
        .unwrap_or_else(|| DEFAULT_ACCOUNT.to_string());
    if let Ok(password) = env::var(PASSWORD_ENV) {
        if !password.is_empty() {
            return AuthCredentials {
                account,
                password: Some(password),
                password_source: "environment",
            };
        }
    }
    AuthCredentials {
        account,
        password: password_fallback.filter(|value| !value.is_empty()).map(str::to_owned),
        password_source: if password_fallback.is_some_and(|value| !value.is_empty()) {
            "runtime_state"
        } else {
            "missing"
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_ACCOUNT, load};

    #[test]
    fn defaults_to_real_account_without_exposing_password() {
        let credentials = load(None, None);
        assert_eq!(credentials.account, DEFAULT_ACCOUNT);
        assert_eq!(credentials.password, None);
        assert_eq!(credentials.password_source, "missing");
    }

    #[test]
    fn keeps_existing_password_as_compatibility_fallback() {
        let credentials = load(None, Some("secret"));
        assert_eq!(credentials.account, DEFAULT_ACCOUNT);
        assert_eq!(credentials.password.as_deref(), Some("secret"));
        assert_eq!(credentials.password_source, "runtime_state");
    }
}
