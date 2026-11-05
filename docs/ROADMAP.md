# Critérios de conclusão e evolução

## Alvo de conclusão

“Completo” neste projeto significa um SO educacional mínimo para x86_64,
executável em QEMU, de usuário único e sem promessa de compatibilidade com PCs
reais. Não significa um SO de propósito geral. O simulador host permanece um
produto didático separado do kernel bootável.

A conclusão exige todos os critérios abaixo:

1. A imagem inicia por BIOS e UEFI; o kernel registra a inicialização pela
   serial e falhas de boot/faults têm diagnóstico observável.
2. O kernel instala GDT/TSS e IDT próprias, trata exceções sem travar em loop
   silencioso e testa faults deliberados.
3. Há gerenciamento próprio de frames e memória virtual; alocações e
   mapeamentos inválidos falham de forma controlada e têm testes de limites.
4. Um timer e controlador de interrupções próprios permitem preempção; dois
   processos demonstram alternância por quantum no QEMU.
5. Código de usuário executa em ring 3, isolado do kernel, e usa uma ABI de
   syscalls documentada e testada.
6. Console e teclado permitem interação; um shell de usuário inicia pelo
   processo inicial.
7. Uma imagem de disco em QEMU permite criar, ler e gravar arquivos; corrupção,
   disco ausente e erros de I/O são detectados sem corromper o kernel.
8. Cada funcionalidade tem testes automatizados adequados; `fmt`, testes,
   Clippy, builds host e freestanding passam, e CI Windows fica verde.
9. Os boots BIOS e UEFI e os testes de interação são executados em QEMU pela
   CI; a documentação só declara combinações realmente verificadas.

Suporte a rede, SMP, múltiplas arquiteturas, hardware físico e segurança de
produção estão fora desse alvo. “100%” só será declarado quando cada critério
acima tiver evidência reproduzível.

## Etapas e critérios de aceite

| Etapa | Entrega | Critério de aceite |
|---|---|---|
| 0. Baseline e publicação | Especificação, workflow e fonte versionada | `cargo fmt --all -- --check`, `cargo test --locked`, Clippy com `-D warnings`, builds host/kernel e workflow GitHub verde |
| 1. Diagnóstico de CPU | Serial inicializada antes do diagnóstico; GDT/TSS e IDT próprias; handlers para exceções x86_64 | Faults de divisão por zero e page fault intencional registram vetor/endereço e encerram sem reset; teste QEMU observa ambos |
| 2. Memória | Mapa físico, alocador de frames e page tables próprias | Testes de alinhamento, esgotamento, liberação e permissões; memória de kernel/user não se sobrepõe |
| 3. Interrupções e tempo | Controlador de interrupções e timer | Tick monotônico, EOI correto e teste QEMU de interrupções sem perda |
| 4. Execução concorrente | Context switch e scheduler preemptivo de kernel | Dois workers progridem sob quantum finito; estado e pilhas preservados |
| 5. Espaço de usuário | Ring 3, isolamento e entrada/retorno de syscall | Programa usermode não acessa memória do kernel; syscalls válidas e inválidas testadas |
| 6. Processos e ABI | Criação/saída de processos, descritores e syscalls documentadas | Testes de ciclo de vida, limites e falhas; sem vazamentos de frames/descritores |
| 7. I/O interativo | Console, teclado, init e shell | Entrada por teclado executa comandos de usuário em QEMU com resultado serial verificável |
| 8. Persistência | Driver de disco virtual e filesystem | Operações de arquivo persistem após reinício QEMU; imagem inválida falha de forma explícita |
| 9. Release | Docs, imagem reproduzível e CI completa | Todos os critérios de conclusão passam em runner Windows; limitações e artefatos versionados documentados |

As etapas são sequenciais quando dependem de hardware previamente implementado.
Uma etapa não é considerada concluída por compilar apenas: exige os testes do
seu critério de aceite. O estado da execução remota depende de um repositório
GitHub; até ele existir, CI e boot em QEMU permanecem pendentes.

## Baseline local conhecido

- Simulador: testes, Clippy, formatação e build release passaram no Windows.
- Kernel freestanding: build release para `x86_64-unknown-none` passou.
- Etapa 1 em andamento: o panic handler reporta mensagem e local pela serial;
  GDT/TSS, IDT e handlers de faults ainda não foram implementados.
- ISO já gerada anteriormente, mas não inicializada em QEMU neste PC.
- A validação GitHub Actions não foi executada: a pasta de trabalho não tem
  repositório Git ou remote associado.
- Nenhuma ferramenta deve ser instalada no computador local; ferramentas de
  boot e emulação podem ser provisionadas somente em runners hospedados.
