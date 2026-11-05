//! **toy-kernel** — simulador educacional de um mini-kernel (catálogo 3.1).
//!
//! Roda **no host** (não é freestanding / não sobe no QEMU neste MVP).
//! Oferece PCB, scheduler RR/prioridade, tabela de syscalls, console VGA-like,
//! ramfs e alocador de páginas simulados.
//!
//! Ver `LIMITATIONS.md` e `boot/README.md` para o que **não** é claim de boot.

pub mod console;
pub mod error;
pub mod kernel;
pub mod mem;
pub mod pcb;
pub mod programs;
pub mod ramfs;
pub mod sched;
pub mod syscall;

pub use console::Console;
pub use error::{KernelError, KernelResult};
pub use kernel::{Kernel, KernelConfig, KernelStats};
pub use pcb::{Pcb, Pid, Priority, ProcessState, UserOp};
pub use programs::ProgId;
pub use sched::{Policy, SchedConfig, Scheduler};
pub use syscall::{SyscallEntry, SYSCALL_TABLE};
pub mod pid;
pub mod pipe;
