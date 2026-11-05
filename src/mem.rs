//! Alocador de páginas e “heap” por processo (brk).

use crate::error::{KernelError, KernelResult};

/// Tamanho de página educativa (4 KiB, igual x86).
pub const PAGE_SIZE: usize = 4096;

/// Frame físico simulado.
#[derive(Debug, Clone)]
pub struct Frame {
    pub id: usize,
    pub used: bool,
    pub owner_pid: Option<u32>,
}

/// Pool de frames + estatísticas.
#[derive(Debug, Clone)]
pub struct PageAllocator {
    frames: Vec<Frame>,
    free_count: usize,
}

impl PageAllocator {
    pub fn new(num_frames: usize) -> Self {
        let frames = (0..num_frames)
            .map(|id| Frame {
                id,
                used: false,
                owner_pid: None,
            })
            .collect();
        Self {
            frames,
            free_count: num_frames,
        }
    }

    pub fn free_pages(&self) -> usize {
        self.free_count
    }

    pub fn total_pages(&self) -> usize {
        self.frames.len()
    }

    pub fn used_pages(&self) -> usize {
        self.frames.len() - self.free_count
    }

    /// Aloca `n` páginas contíguas logicamente (não exige contiguidade física aqui).
    pub fn alloc(&mut self, n: usize, pid: u32) -> KernelResult<Vec<usize>> {
        if n == 0 {
            return Ok(Vec::new());
        }
        if n > self.free_count {
            return Err(KernelError::OutOfMemory {
                requested: n.saturating_mul(PAGE_SIZE),
                free: self.free_count.saturating_mul(PAGE_SIZE),
            });
        }
        let mut ids = Vec::with_capacity(n);
        for f in self.frames.iter_mut() {
            if !f.used {
                f.used = true;
                f.owner_pid = Some(pid);
                ids.push(f.id);
                self.free_count -= 1;
                if ids.len() == n {
                    break;
                }
            }
        }
        Ok(ids)
    }

    pub fn free_pid(&mut self, pid: u32) -> usize {
        let mut freed = 0;
        for f in self.frames.iter_mut() {
            if f.owner_pid == Some(pid) {
                f.used = false;
                f.owner_pid = None;
                freed += 1;
                self.free_count += 1;
            }
        }
        freed
    }

    /// Libera até `n` frames pertencentes ao processo.
    pub fn free_pid_pages(&mut self, pid: u32, n: usize) -> usize {
        let mut freed = 0;
        for frame in &mut self.frames {
            if freed == n {
                break;
            }
            if frame.owner_pid == Some(pid) {
                frame.used = false;
                frame.owner_pid = None;
                freed += 1;
                self.free_count += 1;
            }
        }
        freed
    }
}

/// Calcula páginas necessárias para crescer brk de `old` → `new` bytes.
pub fn pages_for_brk(old_brk: usize, new_brk: usize) -> usize {
    let page_count = |brk: usize| {
        if brk == 0 {
            0
        } else {
            1 + (brk - 1) / PAGE_SIZE
        }
    };
    let old_pages = page_count(old_brk);
    let new_pages = page_count(new_brk);
    new_pages.saturating_sub(old_pages)
}
