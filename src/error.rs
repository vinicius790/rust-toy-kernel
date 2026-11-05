//! Erros do kernel de brinquedo.

use thiserror::Error;

/// Resultado padrão das operações do kernel.
pub type KernelResult<T> = Result<T, KernelError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KernelError {
    #[error("syscall inválida: número {0}")]
    InvalidSyscall(u32),

    #[error("argumento inválido: {0}")]
    InvalidArg(String),

    #[error("processo não encontrado: pid={0}")]
    NoSuchProcess(u32),

    #[error("sem memória: pedido={requested} livre={free}")]
    OutOfMemory { requested: usize, free: usize },

    #[error("arquivo não encontrado: {0}")]
    NoSuchFile(String),

    #[error("fd inválido: {0}")]
    BadFd(i32),

    #[error("processo zumbi / já finalizado: pid={0}")]
    ProcessDead(u32),

    #[error("fila de prontos vazia (só idle)")]
    NoRunnable,

    #[error("limite de processos: {0}")]
    ProcessLimit(usize),

    #[error("espaço de identificadores de processo esgotado")]
    PidExhausted,

    #[error("{0}")]
    Msg(String),
}
