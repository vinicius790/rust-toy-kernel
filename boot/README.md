# Kernel freestanding x86_64

`boot/kernel/` é um binário separado do simulador host. Ele usa `no_std`, inicia
pelo protocolo Limine, escreve `BOOT_OK` na UART COM1 e para a CPU. Ainda não
porta scheduler, syscalls nem processos simulados para hardware; veja
[LIMITATIONS.md](../LIMITATIONS.md).

## Build e smoke test

Pré-requisitos:

- Rust/Cargo 1.85+ e `rustup target add x86_64-unknown-none`;
- Python 3.9+ para executar o helper;
- `xorriso` e, para executar o teste, QEMU com o diretório `share/qemu` e o
  firmware `edk2-x86_64-code.fd` disponíveis ao lado da instalação.

Na raiz do projeto:

```bash
python tools/boot.py
python tools/boot.py --run
```

O script baixa os binários fixados do Limine v12.9.6, compila `toy-kernel-boot`,
monta uma ISO híbrida BIOS/UEFI em `boot/build/toy-kernel.iso` e instala o stage
BIOS com o utilitário Limine. `--run` inicia a ISO em QEMU nos modos BIOS e UEFI,
captura a saída serial e exige o marcador `BOOT_OK` em ambos. O workflow de CI
executa esse teste em um runner Windows e prepara nele as ferramentas
necessárias; nenhuma ferramenta é instalada no computador local. Para iniciar
manualmente, encaminhe COM1 para o terminal (por exemplo, `-serial stdio`).

O alvo pode ser compilado diretamente sem produzir uma ISO:

```bash
rustup target add x86_64-unknown-none
cargo build --locked --package toy-kernel-boot --release --target x86_64-unknown-none
```

O script requer acesso de rede na primeira execução para baixar a distribuição
binária do Limine. Os artefatos ficam sob `boot/build/` e não são adicionados ao
repositório.

`multiboot2_header.S` continua somente como referência didática antiga; não é
montado, linkado ou usado no caminho Limine.
