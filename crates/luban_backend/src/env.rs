use anyhow::anyhow;
use std::path::PathBuf;

#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(test)]
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn lock_env_for_tests() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

pub(crate) fn home_dir() -> anyhow::Result<PathBuf> {
    if let Some(home) = std::env::var_os("HOME")
        && !home.is_empty()
    {
        return Ok(PathBuf::from(home));
    }

    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE")
            && !profile.is_empty()
        {
            return Ok(PathBuf::from(profile));
        }

        let drive = std::env::var_os("HOMEDRIVE");
        let path = std::env::var_os("HOMEPATH");
        if let (Some(drive), Some(path)) = (drive, path)
            && !drive.is_empty()
            && !path.is_empty()
        {
            let combined = format!("{}{}", drive.to_string_lossy(), path.to_string_lossy());
            if !combined.trim().is_empty() {
                return Ok(PathBuf::from(combined));
            }
        }

        Err(anyhow!("HOME/USERPROFILE/HOMEDRIVE+HOMEPATH is not set"))
    }

    #[cfg(not(windows))]
    Err(anyhow!("HOME is not set"))
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
    use crate::test_support::EnvVarGuard;
    use std::path::PathBuf;

    #[test]
    fn home_dir_errors_when_unset() {
        let _guard = lock_env_for_tests();

        let _env = EnvVarGuard::remove("HOME");
        #[cfg(windows)]
        let _env_userprofile = EnvVarGuard::remove("USERPROFILE");
        #[cfg(windows)]
        let _env_homedrive = EnvVarGuard::remove("HOMEDRIVE");
        #[cfg(windows)]
        let _env_homepath = EnvVarGuard::remove("HOMEPATH");

        let err = home_dir().expect_err("missing HOME should error");
        #[cfg(windows)]
        assert!(
            err.to_string()
                .contains("HOME/USERPROFILE/HOMEDRIVE+HOMEPATH is not set"),
            "unexpected error: {err:?}"
        );
        #[cfg(not(windows))]
        assert!(
            err.to_string().contains("HOME is not set"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn home_dir_returns_value() {
        let _guard = lock_env_for_tests();

        let _env = EnvVarGuard::set("HOME", "luban-home");

        let loaded = home_dir().expect("HOME should be read");
        assert_eq!(loaded, PathBuf::from("luban-home"));
    }

    #[cfg(windows)]
    #[test]
    fn home_dir_falls_back_to_userprofile() {
        let _guard = lock_env_for_tests();

        let _env_home = EnvVarGuard::remove("HOME");
        let _env_userprofile = EnvVarGuard::set("USERPROFILE", r"C:\Users\luban");
        let _env_homedrive = EnvVarGuard::remove("HOMEDRIVE");
        let _env_homepath = EnvVarGuard::remove("HOMEPATH");

        let loaded = home_dir().expect("USERPROFILE should be read");
        assert_eq!(loaded, PathBuf::from(r"C:\Users\luban"));
    }

    #[cfg(windows)]
    #[test]
    fn home_dir_falls_back_to_home_drive_path() {
        let _guard = lock_env_for_tests();

        let _env_home = EnvVarGuard::remove("HOME");
        let _env_userprofile = EnvVarGuard::remove("USERPROFILE");
        let _env_homedrive = EnvVarGuard::set("HOMEDRIVE", "C:");
        let _env_homepath = EnvVarGuard::set("HOMEPATH", r"\Users\luban");

        let loaded = home_dir().expect("HOMEDRIVE+HOMEPATH should be read");
        assert_eq!(loaded, PathBuf::from(r"C:\Users\luban"));
    }

    #[test]
    fn optional_trimmed_path_from_env_returns_none_when_unset() {
        let _guard = lock_env_for_tests();

        let _env = EnvVarGuard::remove("LUBAN_TEST_TRIMMED_PATH_ENV");

        let loaded = optional_trimmed_path_from_env("LUBAN_TEST_TRIMMED_PATH_ENV")
            .expect("unset env should not error");
        assert!(loaded.is_none());
    }

    #[test]
    fn optional_trimmed_path_from_env_errors_on_empty() {
        let _guard = lock_env_for_tests();

        let _env = EnvVarGuard::set("LUBAN_TEST_TRIMMED_PATH_ENV", "   ");

        let err = optional_trimmed_path_from_env("LUBAN_TEST_TRIMMED_PATH_ENV")
            .expect_err("empty env should error");
        assert!(
            err.to_string()
                .contains("LUBAN_TEST_TRIMMED_PATH_ENV is set but empty"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn optional_trimmed_path_from_env_trims_value() {
        let _guard = lock_env_for_tests();

        let _env = EnvVarGuard::set("LUBAN_TEST_TRIMMED_PATH_ENV", " luban-test ");

        let loaded = optional_trimmed_path_from_env("LUBAN_TEST_TRIMMED_PATH_ENV")
            .expect("non-empty env should succeed");
        assert_eq!(loaded, Some(PathBuf::from("luban-test")));
    }
}
