# PhantomPOS — Especificação

17/11/2026 · Tegra Software

## Visão geral

O PhantomPOS é uma impressora térmica virtual: um aplicativo desktop que escuta na porta TCP 9100, interpreta comandos ESC/POS e mostra na tela o cupom que sairia no papel.

Objetivos da primeira versão:

- Testar a impressão de sistemas de PDV sem impressora física.
- Ser publicado como projeto de código aberto da Tegra Software.
- Entregar executáveis para Windows, macOS e Linux, gerados por GitHub Actions a cada tag.

O público é quem desenvolve PDV e precisa ver, depurar e validar o que o sistema envia à impressora.

## Escopo

A v1 é um único aplicativo desktop, sem servidor web e sem modo de linha de comando.

**Dentro da v1**

- Aplicativo Tauri 2 com janela única.
- Listener TCP raw na porta 9100, configurável.
- Parser ESC/POS no padrão Epson.
- Renderização da bobina em tempo real.
- Simulação de estados da impressora, com resposta aos comandos de status.
- Log de comandos decodificados.
- Exportação do cupom em PNG e dos bytes brutos.
- Publicação automatizada para os três sistemas.

**Fora da v1**

- Interface web, servidor HTTP ou WebSocket.
- CLI, modo headless e imagem Docker.
- USB, serial e Bluetooth.
- Instalação como impressora do sistema (CUPS, spooler do Windows).
- Várias impressoras virtuais ao mesmo tempo.
- Comandos proprietários de Bematech, Elgin e Daruma.
- Histórico de cupons preservado entre execuções.
- Atualização automática do aplicativo.

Ideias fora do escopo vão para `docs/backlog.md`, sem implementação.

## Identidade

Os projetos abertos da Tegra usam o segmento `oss` no identificador, separado dos produtos comerciais (`app.tegra.<produto>`).

| Item | Valor |
| --- | --- |
| Nome de exibição | PhantomPOS |
| Identificador do app | `app.tegra.oss.phantompos` |
| Binário | `phantompos` |
| Crate do parser | `phantompos-core` |
| Crate do aplicativo | `phantompos` |
| Repositório | `tegrasw/phantompos` |
| Versão inicial | 0.1.0 |
| Licença | MIT |

O identificador é permanente: ele define a pasta de configurações e a identidade do app no macOS. Não deve ser alterado depois da primeira publicação.

## Arquitetura

Um workspace Cargo com dois crates: o parser isolado, sem rede nem disco, e o aplicativo Tauri que o usa.

```
phantompos/
├── core/                    # phantompos-core: parser + modelo do cupom
│   ├── src/
│   └── tests/fixtures/      # capturas .bin + JSON esperado
├── app/
│   ├── src-tauri/           # listener TCP, estado, eventos, configurações
│   └── src/                 # interface: Svelte + TypeScript + Vite
├── docs/backlog.md
├── CLAUDE.md
└── .github/workflows/       # ci.yml e release.yml
```

**Responsabilidades**

| Parte | Faz | Não faz |
| --- | --- | --- |
| `core` | Converte bytes em comandos tipados e monta o modelo do cupom | Rede, disco, Tokio, Tauri |
| `app/src-tauri` | Escuta na 9100, mantém o estado simulado, responde status, emite eventos | Interpretar ESC/POS |
| `app/src` | Desenha a bobina em Canvas, exibe log, estado e configurações | Falar com a rede |

**Fluxo de dados**

1. O listener (Tokio) aceita a conexão e repassa os bytes ao parser.
2. O parser, uma máquina de estados incremental, produz comandos tipados.
3. Consultas de status são respondidas no mesmo socket, conforme o estado simulado.
4. A cada corte ou fim de conexão com conteúdo pendente, o cupom é fechado.
5. O backend envia o cupom e os comandos à janela por eventos do Tauri.
6. A interface desenha o cupom na largura real em pontos.

**Regras do parser**

- Os bytes podem chegar fragmentados em qualquer ponto. O resultado deve ser idêntico ao da entrada inteira.
- Entrada malformada nunca causa pânico. Sequências desconhecidas viram um comando `Unknown` com os bytes preservados e aparecem no log.
- Duas saídas: a lista de comandos (para o log) e o modelo do cupom (para o desenho). Ambas serializáveis em JSON com `serde`.

**Contrato entre backend e interface**

| Tipo | Nome | Conteúdo |
| --- | --- | --- |
| Evento | `receipt:completed` | Cupom fechado, com seus elementos |
| Evento | `commands:received` | Lote de comandos decodificados e bytes correspondentes |
| Evento | `connection:changed` | Conexões ativas e endereço de origem |
| Evento | `drawer:kicked` | Pulso de abertura de gaveta |
| Evento | `listener:status` | Escutando, ou erro (porta em uso) |
| Comando | `get_state` | Estado atual, endereços e configurações |
| Comando | `set_printer_status` | Altera os estados simulados |
| Comando | `update_settings` | Porta, modo de rede, papel, code page padrão |
| Comando | `export_receipt` | Salva PNG ou bytes brutos |

Os nomes são uma proposta; a forma final fica a critério da implementação, desde que documentada no `CLAUDE.md`.

## Comandos ESC/POS

A v1 cobre o conjunto que as bibliotecas de impressão mais usam. Antes de implementar cada comando, conferir parâmetros e valores na [referência de comandos da Epson](https://reference.epson-biz.com/modules/ref_escpos/index.php?content_id=2); esta tabela indica o que entra, não substitui a referência.

| Grupo | Comandos | Etapa |
| --- | --- | --- |
| Inicialização | `ESC @` | 2 |
| Avanço de papel | `LF`, `CR`, `ESC d`, `ESC J`, `ESC 2`, `ESC 3` | 2 |
| Estilo de texto | `ESC E` (negrito), `ESC -` (sublinhado), `GS B` (reverso), `ESC M` (fonte A/B), `GS !` (largura e altura), `ESC !` (modo combinado) | 2 |
| Alinhamento | `ESC a` | 2 |
| Code page | `ESC t` | 2 |
| Corte | `GS V`, `ESC i`, `ESC m` | 3 |
| Gaveta | `ESC p` | 3 |
| Imagem raster | `GS v 0` | 4 |
| QR Code | `GS ( k` | 4 |
| Código de barras | `GS k`, com `GS h`, `GS w` e `GS H` | 4 |
| Status | `DLE EOT`, `GS r`, `GS a` | 5 |

Comandos reconhecidos mas sem efeito visual (por exemplo `ESC R`) são aceitos, registrados no log e ignorados no desenho.

**Code pages**

CP437 como padrão, mais CP850, CP860, CP858 e Windows-1252. São as usadas para português.

**Perfis de papel**

| Papel | Largura em pontos | Colunas na fonte A |
| --- | --- | --- |
| 58 mm | 384 | 32 |
| 80 mm | 576 | 48 |

A fonte A ocupa 12 × 24 pontos e a fonte B, 9 × 17. O padrão do app é 80 mm.

## Interface

Uma janela única com a bobina no centro, cabeçalho fixo e painéis laterais recolhíveis.

| Área | Conteúdo |
| --- | --- |
| Cabeçalho | Endereço de escuta (IP e porta) com botão de copiar, indicador de escuta ou erro, número de conexões ativas |
| Bobina | Cupons empilhados na ordem de chegada, com o corte visível entre eles e rolagem automática para o mais recente |
| Painel de estado | Chaves para sem papel, tampa aberta e offline; indicador de gaveta aberta |
| Log de comandos | Hexadecimal e comando decodificado lado a lado, com destaque para sequências desconhecidas |
| Configurações | Porta, aceitar conexões só locais ou da rede, largura do papel, code page padrão |
| Ações | Limpar a bobina, salvar o cupom como PNG, exportar os bytes brutos |

**Endereço no cabeçalho**

- No modo local, mostra `127.0.0.1:9100`.
- No modo rede, lista todos os endereços IPv4 da máquina que não sejam loopback, cada um com a porta.
- Se a porta estiver ocupada, mostra o erro no lugar do endereço, sem derrubar o app.

**Desenho do cupom**

- Canvas na largura exata do papel em pontos, ampliado para a tela sem suavização.
- Fonte monoespaçada embutida no app, ajustada à célula de cada fonte da impressora.
- QR Code e códigos de barras gerados por bibliotecas JavaScript.
- Fidelidade ponto a ponto com uma impressora real não é requisito da v1. Colunas, quebras de linha, alinhamento e tamanhos devem bater.

**Aparência**

Tema claro e escuro, seguindo o sistema. Interface em português e inglês, conforme o idioma do sistema. A exportação dos bytes brutos gera um arquivo `.bin` que serve direto como caso de teste em `core/tests/fixtures/`.

## Rede e status

O app escuta só em `127.0.0.1:9100` por padrão e passa a aceitar a rede local quando o usuário habilita essa opção.

**Listener**

- Protocolo TCP raw, sem autenticação nem criptografia, como numa impressora real.
- Modo local: `127.0.0.1`. Modo rede: `0.0.0.0`. A troca reinicia o listener sem fechar o app.
- Ao habilitar o modo rede, a interface avisa que qualquer dispositivo da rede poderá enviar impressões.
- Várias conexões simultâneas são aceitas, cada uma com seu parser. Os cupons entram na mesma bobina na ordem em que são fechados.
- Um cupom fecha em um comando de corte ou no fim da conexão, se houver conteúdo pendente.

**Estados simulados**

| Estado | Efeito na impressão | Efeito no status |
| --- | --- | --- |
| Normal | Imprime | Reporta online, com papel |
| Sem papel | Dados registrados no log, nada é impresso | Reporta fim de papel |
| Tampa aberta | Dados registrados no log, nada é impresso | Reporta tampa aberta |
| Offline | Dados registrados no log, nada é impresso | Reporta offline |
| Gaveta aberta | Não afeta a impressão | Reporta o sinal da gaveta |

As respostas a `DLE EOT` e `GS r` seguem os bytes definidos na referência da Epson. Com `GS a` habilitado pelo cliente, o app envia o status automaticamente a cada mudança de estado.

## Testes

A qualidade do parser é garantida por capturas de referência: cada arquivo de bytes tem um JSON esperado, e todo comando novo precisa passar por todas elas.

**No `core`**

- Cada caso é um par `nome.bin` e `nome.json` em `core/tests/fixtures/`. O JSON traz a lista de comandos e o modelo do cupom.
- Um único teste percorre todos os pares. Com `UPDATE_FIXTURES=1`, ele regrava os JSON, e a diferença é revisada no commit.
- Teste de fragmentação: cada captura é processada inteira e byte a byte, e os dois resultados precisam ser iguais.
- Teste de robustez: entradas aleatórias nunca causam pânico.
- Origem das capturas: bytes gerados por bibliotecas ESC/POS conhecidas e, a partir da etapa 6, cupons reais exportados pelo próprio app.

**No aplicativo**

- Teste de fumaça manual por etapa, enviando bytes com `nc` para a 9100.
- Sem testes automatizados de interface na v1.

**Em todo pull request (`ci.yml`)**

- `cargo fmt --check` e `cargo clippy` sem avisos.
- `cargo test` no workspace.
- Checagem de tipos e build do frontend.

## Build e publicação

Um workflow `release.yml`, disparado por tags `v*`, usa a action oficial do Tauri numa matriz de três sistemas e anexa os artefatos a um rascunho de release no GitHub.

| Sistema | Runner | Artefatos |
| --- | --- | --- |
| Windows | `windows-latest` | Instalador NSIS (`.exe`) |
| macOS | `macos-latest`, alvos `aarch64` e `x86_64` | `.dmg` assinado e notarizado |
| Linux | `ubuntu-22.04` | AppImage e `.deb` |

**Assinatura no macOS**

A assinatura e a notarização rodam no próprio runner, com o certificado da Tegra Software em secrets. Não há build local nem Xcode Cloud.

| Secret | Conteúdo |
| --- | --- |
| `APPLE_CERTIFICATE` | Certificado Developer ID Application exportado como `.p12`, em base64 |
| `APPLE_CERTIFICATE_PASSWORD` | Senha definida na exportação |
| `APPLE_SIGNING_IDENTITY` | Identidade de assinatura |
| `KEYCHAIN_PASSWORD` | Senha do keychain temporário do runner |
| `APPLE_API_ISSUER` | Issuer da chave do App Store Connect |
| `APPLE_API_KEY` | ID da chave |
| `APPLE_API_KEY_PATH` | Caminho do arquivo da chave privada no runner |

- O certificado precisa ser do tipo Developer ID Application, criado pelo Account Holder da conta.
- O workflow de exemplo da documentação procura a identidade pelo texto "Apple Development". Ajustar para o certificado Developer ID.
- A assinatura roda só no workflow de release, nunca em pull requests.

Fonte: [macOS Code Signing, documentação do Tauri](https://v2.tauri.app/distribute/sign/macos/).

**Windows e Linux**

- Windows sai sem assinatura na v1, então o SmartScreen exibe aviso na primeira execução.
- No Linux, o AppImage embute as dependências e o `.deb` declara o WebKitGTK como requisito.

## Etapas e critérios de aceite

São seis etapas em sequência. Uma etapa só começa quando os critérios da anterior estão atendidos e o CI está verde.

| Etapa | Entrega | Critérios de aceite |
| --- | --- | --- |
| 1. Esqueleto | Workspace, janela Tauri, listener na 9100, bytes brutos no log, `ci.yml` e `release.yml` | Enviar bytes com `nc` mostra o hexadecimal na janela; uma tag de teste gera os artefatos dos três sistemas; o `.dmg` abre sem alerta do Gatekeeper |
| 2. Texto e formatação | Parser dos grupos da etapa 2, modelo do cupom, estrutura de fixtures | Fixtures de texto, estilos, alinhamento e code pages passam; testes de fragmentação e robustez passam |
| 3. Bobina, corte e gaveta | Desenho em Canvas, fechamento do cupom, indicador de gaveta | Um cupom de texto aparece com colunas e quebras corretas em 58 e 80 mm; o corte separa cupons; `ESC p` aciona o indicador |
| 4. Imagem e códigos | `GS v 0`, QR Code e códigos de barras | Logo raster, QR Code e EAN-13 de um cupom de exemplo aparecem; o QR Code é lido por um celular |
| 5. Status | Painel de estado e respostas a `DLE EOT`, `GS r` e `GS a` | Em cada estado, a resposta confere com a referência da Epson; em estados de erro nada é impresso |
| 6. Configurações e exportação | Porta, modo de rede, endereços no cabeçalho, papel, exportação em PNG e `.bin` | Um segundo dispositivo imprime pelo IP exibido; a troca de porta funciona sem reiniciar; o `.bin` exportado passa como fixture |

A versão 0.1.0 é publicada ao fim da etapa 6, depois de um teste com cupons reais de um PDV.

## Convenções para o Claude Code

O desenvolvimento será feito pelo Claude Code, uma etapa por vez, com esta especificação como referência.

- **Versões atuais.** Antes de escrever código de Tauri, Tokio, Svelte ou da action de release, consultar a documentação vigente. Não confiar na memória para APIs.
- **Escopo.** Não implementar nada da lista "Fora da v1". Ideias vão para `docs/backlog.md`.
- **Idioma do código.** Código, comentários, mensagens de commit e README em inglês.
- **Erros.** Sem `unwrap` ou `expect` em caminhos que tratam dados da rede. Erros tipados no `core`.
- **Isolamento do `core`.** Nenhuma dependência de rede, disco, Tokio ou Tauri.
- **Dependências.** Cada crate ou pacote novo precisa de justificativa na descrição do commit.
- **Commits.** Pequenos, um por unidade lógica, no padrão Conventional Commits.
- **Testes primeiro no parser.** Todo comando novo chega com sua fixture.
- **Dúvida sobre ESC/POS.** Vale a referência da Epson. Desvios deliberados são documentados no código.
- **`CLAUDE.md`.** Manter atualizado com os comandos de build, teste e execução, e com o contrato de eventos entre backend e interface.
- **Fim de etapa.** Relatar os critérios atendidos, o que ficou pendente e as decisões tomadas sem consulta.

## Decisões e pendências

As sete decisões foram tomadas em 17/11/2026. Restam duas tarefas do mantenedor antes da primeira release.

| Tema | Decisão |
| --- | --- |
| Licença | MIT |
| Repositório | `tegrasw/phantompos` |
| Transporte | Rede, TCP 9100 |
| Nome | Considerado disponível; confirmação pendente |
| Idiomas da interface | Português e inglês |
| Certificado da Apple | O mantenedor é o Account Holder e cria o certificado |
| Assinatura no Windows | Sem assinatura |

**Pendências**

- [ ] Confirmar `phantompos` e `phantompos-core` no crates.io antes de publicar.
- [ ] Criar o certificado Developer ID Application e a chave do App Store Connect, e cadastrar os secrets no repositório, antes da tag de teste da etapa 1.
