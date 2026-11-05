//! Filesystem mínimo em RAM (ramdisk educativo).

use std::collections::HashMap;

use crate::error::{KernelError, KernelResult};

#[derive(Debug, Clone, Default)]
pub struct RamFs {
    files: HashMap<String, Vec<u8>>,
}

impl RamFs {
    pub fn new() -> Self {
        let mut fs = Self {
            files: HashMap::new(),
        };
        // Dispositivos virtuais (metadados; I/O real vai pelo console).
        fs.files.insert("/dev/stdin".into(), Vec::new());
        fs.files.insert("/dev/stdout".into(), Vec::new());
        fs.files.insert("/dev/stderr".into(), Vec::new());
        fs.files
            .insert("/etc/motd".into(), b"toy-kernel 0.1 - bem-vindo\n".to_vec());
        fs
    }

    pub fn create(&mut self, path: &str, data: impl Into<Vec<u8>>) {
        self.files.insert(path.to_string(), data.into());
    }

    pub fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn read(&self, path: &str) -> KernelResult<&[u8]> {
        self.files
            .get(path)
            .map(|v| v.as_slice())
            .ok_or_else(|| KernelError::NoSuchFile(path.to_string()))
    }

    pub fn write(&mut self, path: &str, data: &[u8]) -> KernelResult<()> {
        let entry = self
            .files
            .get_mut(path)
            .ok_or_else(|| KernelError::NoSuchFile(path.to_string()))?;
        entry.clear();
        entry.extend_from_slice(data);
        Ok(())
    }

    pub fn append(&mut self, path: &str, data: &[u8]) -> KernelResult<()> {
        let entry = self
            .files
            .get_mut(path)
            .ok_or_else(|| KernelError::NoSuchFile(path.to_string()))?;
        entry.extend_from_slice(data);
        Ok(())
    }

    pub fn list(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.files.keys().cloned().collect();
        keys.sort();
        keys
    }
}
