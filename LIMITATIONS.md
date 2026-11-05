# LIMITATIONS — escopo real dos dois alvos

Este repositório contém um simulador executado no host e um kernel freestanding
inicializável. Eles são implementações separadas: o boot do segundo não torna
as abstrações do primeiro funcionalidades de hardware.

## Kernel freestanding (`boot/kernel`)

O alvo `toy-kernel-boot` compila como `no_std` para `x86_64-unknown-none`, usa o
protocolo Limine, escreve mensagens de boot e diagnóstico de panic em COM1 e
para a CPU.
`tools/boot.py --run` cria uma ISO híbrida e verifica a mensagem no QEMU.

Ainda **não** é um sistema operacional completo:

| Recurso | Estado |
|---------|--------|
| Entrada x86_64 pelo Limine e saída serial | Implementado |
| Diagnóstico de panic Rust pela serial | Implementado; não trata faults de CPU |
| GDT/IDT próprias, handlers de exceção e interrupções | Não implementados |
| Timer/APIC e preempção por IRQ | Não implementados |
| Page tables / allocator próprios | Não implementados; o bootloader prepara o ambiente exigido pelo protocolo |
| Ring 3, isolamento e instrução de syscall real | Não implementados |
| Scheduler, PCB, syscalls e userland do simulador | Não portados para o alvo freestanding |
| VGA/PS/2, filesystem em disco e initramfs | Não implementados |

## Simulador host (`src/`)

O binário `toy-kernel` roda como processo normal do host. Os ticks, processos,
syscalls, console 80×25, ramfs e frames de 4 KiB são modelos em memória Rust:

- não existe concorrência real entre processos simulados;
- `UserOp` não é instrução de CPU nem código de ring 3;
- `read` retorna a contagem limitada por `max`, sem copiar para um buffer de usuário;
- `write` resolve um ID de mensagem para bytes de uma tabela interna;
- `PageAllocator` contabiliza frames, mas não manipula CR3 nem page tables;
- o console não é VGA MMIO e `RamFs` não persiste em disco.

## Artefatos de referência

`boot/multiboot2_header.S` é apenas um exemplo histórico e não participa da ISO
Limine nem do build Rust. A rota executável é descrita em [boot/README.md](./boot/README.md).

## Próximos passos para um SO completo

1. Instalar GDT/TSS, IDT e handlers de exceção; validar faults intencionais.
2. Adicionar timer e scheduler de hardware com contexto de execução.
3. Implementar allocator/page tables próprios e transição segura para ring 3.
4. Definir ABI de syscalls, userland, drivers e filesystem real.
5. Integrar e testar cada etapa em QEMU antes de alegar suporte.
