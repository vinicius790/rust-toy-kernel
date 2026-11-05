# Tecnologias — toy-kernel

## Linguagem e tooling

| Peça | Escolha | Motivo |
|------|---------|--------|
| Linguagem | **Rust 2021** | Segurança de memória no host; bom para PCB/filas tipadas |
| CLI | `clap` 4 (derive) | Subcomandos `run` / `trace` / `demo` / `syscalls` |
| Erros | `thiserror` 2 | `KernelError` legível em docs e testes |
| Testes do simulador | `cargo test --locked` | Testes de integração no host |
| Kernel freestanding | crate `boot/kernel`, `no_std`, target `x86_64-unknown-none` | Binário separado, entrypoint Limine + UART COM1 |
| Imagem/boot | Limine `v12.9.6`, `xorriso`, QEMU | ISO híbrida e smoke test serial BIOS/UEFI na CI Windows |
| Licença | MIT | Alinhado ao restante do catálogo |

## O que **ainda não** está implementado

- GDT/IDT próprias, handlers de exceção, timer/APIC e preempção por IRQ
- allocator e page tables próprios, ring 3 e syscall instruction
- scheduler/PCB/syscalls simulados portados para o alvo freestanding
- VGA MMIO, PS/2, filesystem em disco e initramfs

## Dependências mínimas

O simulador depende de `clap` e `thiserror`; o kernel freestanding depende do
crate `limine` e usa `core` + I/O de portas x86. Sem async ou Tokio. O núcleo
do simulador (`Kernel`, `Scheduler`, `Pcb`) é `std` puro + coleções.

## Como estender

1. Novo syscall: entrada em `SYSCALL_TABLE` + braço em `dispatch_syscall` + teste
2. Novo programa: variante em `ProgId` + `load_program` + nome no CLI
3. Nova política: enum `Policy` + `pick_next`
4. Evoluir o kernel freestanding com IDT/timer e portar componentes após definir
   interfaces `no_std` e validar cada etapa em QEMU.
