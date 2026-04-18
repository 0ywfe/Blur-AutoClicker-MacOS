use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

/// Development logger that writes to a file for debugging
pub struct DevLogger {
    file: Mutex<Option<std::fs::File>>,
    path: PathBuf,
}

impl DevLogger {
    pub fn new(name: &str) -> Self {
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("BlurAutoClicker")
            .join("logs");

        let _ = fs::create_dir_all(&log_dir);

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let path = log_dir.join(format!("{}_{}.log", name, timestamp));

        DevLogger {
            file: Mutex::new(None),
            path,
        }
    }

    fn ensure_file(&self) {
        let mut guard = self.file.lock().unwrap();
        if guard.is_none() {
            if let Ok(file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                *guard = Some(file);
            }
        }
    }

    pub fn log(&self, module: &str, message: &str) {
        self.ensure_file();
        let mut guard = self.file.lock().unwrap();
        if let Some(ref mut file) = *guard {
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| {
                    let secs = d.as_secs();
                    let millis = d.subsec_millis();
                    format!("{}.{:03}", secs, millis)
                })
                .unwrap_or_else(|_| String::from("0.000"));

            let entry = format!("[{}] [{}] {}\n", timestamp, module, message);
            let _ = file.write_all(entry.as_bytes());
            let _ = file.flush();
        }
    }

    #[allow(dead_code)]
    pub fn log_value(&self, module: &str, key: &str, value: &str) {
        self.log(module, &format!("  {} = {:?}", key, value));
    }

    #[allow(dead_code)]
    #[inline]
    pub fn log_enabled(&self) -> bool {
        cfg!(debug_assertions)
    }
}

// Singleton instance for global use
lazy_static::lazy_static! {
    pub static ref DEV_LOGGER: DevLogger = DevLogger::new("blur_autoclicker");
}

// Convenience macros for logging
#[macro_export]
macro_rules! dev_log {
    ($module:expr, $message:expr) => {
        if $crate::dev_logger::DEV_LOGGER.log_enabled() {
            $crate::dev_logger::DEV_LOGGER.log($module, $message);
        }
    };
    ($module:expr, $key:expr, $value:expr) => {
        if $crate::dev_logger::DEV_LOGGER.log_enabled() {
            $crate::dev_logger::DEV_LOGGER.log_value($module, $key, &format!("{:?}", $value));
        }
    };
}

#[macro_export]
macro_rules! dev_log_fn {
    ($module:expr) => {
        if $crate::dev_logger::DEV_LOGGER.log_enabled() {
            $crate::dev_logger::DEV_LOGGER.log(
                $module,
                &format!(">>> {} called", std::any::type_name::<Self>()),
            );
        }
    };
}
