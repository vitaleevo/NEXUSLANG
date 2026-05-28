use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

pub const NEXUS_DATA_DIR_ENV: &str = "NEXUS_DATA_DIR";

thread_local! {
    static THREAD_ENV_OVERRIDES: RefCell<HashMap<OsString, OsString>> =
        RefCell::new(HashMap::new());
}

pub struct RuntimeEnvVarGuard {
    key: OsString,
    previous: Option<OsString>,
}

pub fn set_thread_var_path(key: &'static str, value: &Path) -> RuntimeEnvVarGuard {
    set_thread_var(key, value.as_os_str().to_os_string())
}

pub fn set_thread_var(key: &'static str, value: OsString) -> RuntimeEnvVarGuard {
    let key = OsString::from(key);
    let previous =
        THREAD_ENV_OVERRIDES.with(|overrides| overrides.borrow_mut().insert(key.clone(), value));
    RuntimeEnvVarGuard { key, previous }
}

pub fn var_os(name: &str) -> Option<OsString> {
    let key = OsString::from(name);
    THREAD_ENV_OVERRIDES
        .with(|overrides| overrides.borrow().get(&key).cloned())
        .or_else(|| std::env::var_os(name))
}

pub fn var_string(name: &str) -> Option<String> {
    var_os(name).map(|value| value.to_string_lossy().into_owned())
}

impl Drop for RuntimeEnvVarGuard {
    fn drop(&mut self) {
        THREAD_ENV_OVERRIDES.with(|overrides| {
            let mut overrides = overrides.borrow_mut();
            if let Some(previous) = &self.previous {
                overrides.insert(self.key.clone(), previous.clone());
            } else {
                overrides.remove(&self.key);
            }
        });
    }
}
