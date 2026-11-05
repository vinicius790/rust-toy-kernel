# Implementação e evolução do boot

O caminho freestanding atual está em `boot/kernel/`: alvo Rust
`x86_64-unknown-none`, entrypoint `_start`, diagnóstico serial COM1 e parada com
`hlt`. A configuração Limine e o linker script são usados pelo build de
`toy-kernel-boot`; `tools/boot.py` prepara a ISO e pode validá-la no QEMU.

O bootloader entrega o controle em x86_64 segundo o protocolo Limine. Isso não
significa que o kernel já possua GDT/IDT, interrupções, memória virtual própria,
scheduler ou userland. O simulador em `src/` permanece independente até que
essas partes sejam portadas e testadas uma a uma.
