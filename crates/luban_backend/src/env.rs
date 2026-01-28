use anyhow::anyhow;
use std::path::PathBuf;

#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(test)]
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn lock_env_for_tests() -> MutexGuard<'static, ()> {
    ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

pub(crate) fn home_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home))
}

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
    use super::lock_env_for_tests;
    use super::{home_dir, optional_trimmed_path_from_env};
    use std::path::PathBuf;

    #[test]
    fn home_dir_errors_when_unset() {
        let _guard = lock_env_for_tests();

        let prev = std::env::var_os("HOME");
        unsafe {
            std::env::remove_var("HOME");
        }

        let err = home_dir().expect_err("missing HOME should error");
        assert!(
            err.to_string().contains("HOME is not set"),
            "unexpected error: {err:?}"
        );

        if let Some(value) = prev {
            unsafe {
                std::env::set_var("HOME", value);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn home_dir_returns_value() {
        let _guard = lock_env_for_tests();

        let prev = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("HOME", "luban-home");
        }

        let loaded = home_dir().expect("HOME should be read");
        assert_eq!(loaded, PathBuf::from("luban-home"));

        if let Some(value) = prev {
            unsafe {
                std::env::set_var("HOME", value);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn optional_trimmed_path_from_env_returns_none_when_unset() {
        let _guard = lock_env_for_tests();

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
        let _guard = lock_env_for_tests();

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
        let _guard = lock_env_for_tests();

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
