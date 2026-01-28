use anyhow::anyhow;
use std::path::PathBuf;

pub(crate) fn optional_trimmed_path_from_env(name: &str) -> anyhow::Result<Option<PathBuf>> {
    let value = match std::env::var_os(name) {
        Some(value) => value,
        None => return Ok(None),
    };

    let value = value.to_string_lossy();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("{name} is set but empty"));
    }

    Ok(Some(PathBuf::from(trimmed)))
}

#[cfg(test)]
mod tests {
    use super::optional_trimmed_path_from_env;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn optional_trimmed_path_from_env_returns_none_when_unset() {
        let _guard = lock_env();

        let prev = std::env::var_os("LUBAN_TEST_TRIMMED_PATH_ENV");
        unsafe {
            std::env::remove_var("LUBAN_TEST_TRIMMED_PATH_ENV");
        }

        let loaded = optional_trimmed_path_from_env("LUBAN_TEST_TRIMMED_PATH_ENV")
            .expect("unset env should not error");
        assert!(loaded.is_none());

        if let Some(value) = prev {
            unsafe {
                std::env::set_var("LUBAN_TEST_TRIMMED_PATH_ENV", value);
            }
        } else {
            unsafe {
                std::env::remove_var("LUBAN_TEST_TRIMMED_PATH_ENV");
            }
        }
    }

    #[test]
    fn optional_trimmed_path_from_env_errors_on_empty() {
        let _guard = lock_env();

        let prev = std::env::var_os("LUBAN_TEST_TRIMMED_PATH_ENV");
        unsafe {
            std::env::set_var("LUBAN_TEST_TRIMMED_PATH_ENV", "   ");
        }

        let err = optional_trimmed_path_from_env("LUBAN_TEST_TRIMMED_PATH_ENV")
            .expect_err("empty env should error");
        assert!(
            err.to_string()
                .contains("LUBAN_TEST_TRIMMED_PATH_ENV is set but empty"),
            "unexpected error: {err:?}"
        );

        if let Some(value) = prev {
            unsafe {
                std::env::set_var("LUBAN_TEST_TRIMMED_PATH_ENV", value);
            }
        } else {
            unsafe {
                std::env::remove_var("LUBAN_TEST_TRIMMED_PATH_ENV");
            }
        }
    }

    #[test]
    fn optional_trimmed_path_from_env_trims_value() {
        let _guard = lock_env();

        let prev = std::env::var_os("LUBAN_TEST_TRIMMED_PATH_ENV");
        unsafe {
            std::env::set_var("LUBAN_TEST_TRIMMED_PATH_ENV", " luban-test ");
        }

        let loaded = optional_trimmed_path_from_env("LUBAN_TEST_TRIMMED_PATH_ENV")
            .expect("non-empty env should succeed");
        assert_eq!(loaded, Some(PathBuf::from("luban-test")));

        if let Some(value) = prev {
            unsafe {
                std::env::set_var("LUBAN_TEST_TRIMMED_PATH_ENV", value);
            }
        } else {
            unsafe {
                std::env::remove_var("LUBAN_TEST_TRIMMED_PATH_ENV");
            }
        }
    }
}
