//! Tabela de syscalls e despacho.

use crate::error::KernelError;

/// Números de syscall (ABI estável deste simulador).
pub mod nr {
    pub const WRITE: u32 = 1;
    pub const READ: u32 = 2;
    pub const YIELD: u32 = 3;
    pub const EXIT: u32 = 4;
    pub const GETPID: u32 = 5;
    pub const SLEEP: u32 = 6;
    pub const SPAWN: u32 = 7;
    pub const BRK: u32 = 8;
    pub const OPEN: u32 = 9;
    pub const CLOSE: u32 = 10;
    pub const GETTICKS: u32 = 11;
    pub const KILL: u32 = 12;
    pub const GETPPID: u32 = 13;
    pub const NICE: u32 = 14;
}

/// Entrada da tabela de syscalls (metadados educativos).
#[derive(Debug, Clone, Copy)]
pub struct SyscallEntry {
    pub num: u32,
    pub name: &'static str,
    pub argc: u8,
    pub doc: &'static str,
}

/// Tabela completa — espelho do que o despachante aceita.
pub static SYSCALL_TABLE: &[SyscallEntry] = &[
    SyscallEntry {
        num: nr::WRITE,
        name: "write",
        argc: 3,
        doc: "write(fd, msg_id, len) — len=0 escreve a mensagem inteira",
    },
    SyscallEntry {
        num: nr::READ,
        name: "read",
        argc: 3,
        doc: "read(fd, buf_id, max) — retorna contagem, sem consumir bytes (max=0 = todos)",
    },
    SyscallEntry {
        num: nr::YIELD,
        name: "yield",
        argc: 0,
        doc: "yield() — cede a CPU voluntariamente",
    },
    SyscallEntry {
        num: nr::EXIT,
        name: "exit",
        argc: 1,
        doc: "exit(code) — termina o processo",
    },
    SyscallEntry {
        num: nr::GETPID,
        name: "getpid",
        argc: 0,
        doc: "getpid() → pid",
    },
    SyscallEntry {
        num: nr::SLEEP,
        name: "sleep",
        argc: 1,
        doc: "sleep(ticks) — bloqueia até now+ticks",
    },
    SyscallEntry {
        num: nr::SPAWN,
        name: "spawn",
        argc: 2,
        doc: "spawn(prog_id, priority) → child_pid (prog_id indexa biblioteca interna)",
    },
    SyscallEntry {
        num: nr::BRK,
        name: "brk",
        argc: 1,
        doc: "brk(new_end) → 0 ok / erro OOM",
    },
    SyscallEntry {
        num: nr::OPEN,
        name: "open",
        argc: 1,
        doc: "open(path_id) → fd; máximo de 256 FDs por processo",
    },
    SyscallEntry {
        num: nr::CLOSE,
        name: "close",
        argc: 1,
        doc: "close(fd)",
    },
    SyscallEntry {
        num: nr::GETTICKS,
        name: "getticks",
        argc: 0,
        doc: "getticks() → tick atual do kernel",
    },
    SyscallEntry {
        num: nr::KILL,
        name: "kill",
        argc: 1,
        doc: "kill(pid) — força EXIT do alvo",
    },
    SyscallEntry {
        num: nr::GETPPID,
        name: "getppid",
        argc: 0,
        doc: "getppid() → ppid",
    },
    SyscallEntry {
        num: nr::NICE,
        name: "nice",
        argc: 1,
        doc: "nice(prio) — altera prioridade do processo corrente",
    },
];

pub fn lookup(num: u32) -> Result<&'static SyscallEntry, KernelError> {
    SYSCALL_TABLE
        .iter()
        .find(|e| e.num == num)
        .ok_or(KernelError::InvalidSyscall(num))
}

pub fn name_of(num: u32) -> &'static str {
    lookup(num).map(|e| e.name).unwrap_or("???")
}

/// Retorno de syscall no registrador “a0” simulado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysRet(pub i64);

impl SysRet {
    pub const OK: Self = Self(0);
    pub fn err(code: i64) -> Self {
        Self(code)
    }
}

/// Códigos de erro negativos estilo Linux (simplificados).
pub mod errno {
    pub const EINVAL: i64 = -22;
    pub const ENOENT: i64 = -2;
    pub const ENOMEM: i64 = -12;
    pub const EBADF: i64 = -9;
    pub const ESRCH: i64 = -3;
    pub const EAGAIN: i64 = -11;
    pub const EMFILE: i64 = -24;
}
