# Screen Voice

Agente de voz mínimo para Windows. Ao pressionar uma tecla global, captura o monitor principal, interpreta a imagem com a OpenAI e fala uma orientação curta em português brasileiro. Quando a tela exigir conhecimento de engenharia de IA, o modelo pode pesquisar palestras e transcrições públicas pelo [MCP do AI Engineer](https://ai.engineer/data). Não grava a tela nem escuta o microfone. A voz é gerada por IA.

Falas curtas são reproduzidas na sessão **Sons do Sistema** do Windows. Para falas maiores ou quando esse modo falha, o programa usa a saída de áudio normal do Windows e pode aparecer com controle próprio no mixer durante a reprodução.

## Uso

1. Instale [Rust](https://rustup.rs/) e [crie uma chave real da API OpenAI](https://platform.openai.com/api-keys).
2. Compile uma vez, nesta pasta:

```powershell
cargo build --release
```

3. Configure `OPENAI_API_KEY` nas **Variáveis de ambiente para sua conta** do Windows e abra `target\release\screen-voice.exe` pelo Explorador de Arquivos. O executável abre sem janela. Pressione `Ctrl+Alt+S` para ouvir a análise. Para encerrar, finalize `screen-voice.exe` na guia **Detalhes** do Gerenciador de Tarefas.

O programa registra inicialização, erros e se chamou o MCP em `%LOCALAPPDATA%\screen-voice\screen-voice.log`. Esse arquivo não contém a chave, o texto interpretado da tela nem os termos pesquisados. Se não ouvir resposta, consulte o erro mais recente ali. Guardar `OPENAI_API_KEY` como variável de ambiente permanente da conta facilita a execução sem terminal, mas deixa a chave armazenada no perfil do Windows; proteja sua conta.

As instruções de resposta, pesquisa e fala estão em `skills/` e entram no executável durante a compilação. Veja [as práticas e o teste manual](docs/voice-agent-practices.md) usados para curá-las. O log marca as etapas de cada acionamento sem gravar a captura ou o texto falado.

O MCP do AI Engineer fica disponível por padrão somente com as ferramentas públicas `search` e `fetch`. A imagem vai para a API OpenAI; se o modelo decidir pesquisar, a consulta ao AI Engineer pode conter termos derivados da tela. A instrução do agente limita a pesquisa a termos técnicos genéricos, mas não é uma garantia de privacidade. Para desligar essa conexão, defina `VOICE_AI_ENGINEER_MCP=0` antes de iniciar o programa. O MCP é uma fonte de conhecimento, não o serviço que interpreta imagens.

Configurações opcionais: crie estas variáveis no mesmo painel de ambiente do Windows antes de abrir pelo Explorador. Se iniciar pelo PowerShell, também pode defini-las apenas para aquela sessão:

```powershell
$env:VOICE_HOTKEY = "CTRL+SHIFT+F8"
$env:VOICE_VISION_MODEL = "gpt-5.4-mini"
$env:VOICE_NAME = "marin"
$env:VOICE_AI_ENGINEER_MCP = "0" # opcional: desliga a pesquisa externa
```

`VOICE_HOTKEY` aceita letras, números ou F1–F24, com `CTRL`, `ALT`, `SHIFT` ou `WIN`. O programa captura apenas o monitor principal. Cada acionamento envia uma imagem da tela à API OpenAI e gera áudio pela API; evite acioná-lo quando dados sensíveis estiverem visíveis.

Se a API responder `429`, confira o código detalhado no log: ele distingue falta de créditos ou limite de gastos de um limite temporário de requisições. Nunca compartilhe nem fotografe uma chave de API; se ela aparecer em uma captura, revogue-a e crie outra.

## Versão experimental para entrevista: GPT Live

`screen-voice-live.exe` é um segundo executável. Ele usa o microfone padrão para sua voz e o loopback da saída de áudio padrão do Windows para a voz da videochamada. Envia a mistura ao GPT Live continuamente. A resposta do agente sai apenas no dispositivo de áudio padrão: selecione seu fone como saída padrão e verifique no aplicativo da chamada que o microfone selecionado é o seu microfone físico, não uma entrada virtual que inclua o áudio do agente.

Compile uma vez na pasta do projeto:

```powershell
cargo build --release --bin screen-voice-live
```

Depois, dê dois cliques em `start-screen-voice-live.cmd`. Na primeira execução, cole sua chave da API no prompt local. Ela fica criptografada em `%LOCALAPPDATA%\screen-voice\openai-api-key.clixml` para a sua conta neste computador. Nas próximas vezes, use o mesmo iniciador sem redigitar a chave. Para trocar a chave, execute `.\start-screen-voice-live.cmd -Setup` no PowerShell da pasta do projeto. O iniciador abre o agente sem janela de console; abrir o `.exe` diretamente ainda exige `OPENAI_API_KEY` no ambiente.

O executável não abre console. Ele começa calado e **continua ouvindo enquanto não fala**. Durante uma simulação consentida com um amigo, use:

| Atalho | Ação |
| --- | --- |
| `Ctrl+Alt+H` | Pede o próximo passo de implementação, sem repetir a pista anterior. |
| `Ctrl+Alt+A` | Pede uma resposta mais completa para estudo. |
| `Ctrl+Alt+E` | Captura o monitor principal, identifica o exercício visível e pede o primeiro passo para escrever no código. |
| `Ctrl+Alt+L` | Interrompe a fala atual; a escuta continua. |
| `Ctrl+Alt+Shift+L` | Encerra a sessão e a cobrança. |

A dica espera uma pausa de cerca de meio segundo antes de tocar e é cortada quando uma nova fala é detectada no microfone. Durante a janela da resposta, o loopback da chamada fica suspenso para que a própria voz do agente não interrompa a dica. `Ctrl+Alt+E` envia a captura à API OpenAI somente quando acionado. O registro de estado e erros fica em `%LOCALAPPDATA%\screen-voice\screen-voice-live.log`; não inclui gravações, transcrições ou capturas.

Para avançar no mesmo exercício, use `Ctrl+Alt+H` depois de `Ctrl+Alt+E`. Pressionar `Ctrl+Alt+E` de novo recaptura a tela e pede novamente o primeiro passo.

Para conferir os dispositivos de áudio sem usar a API, execute `.\target\release\screen-voice-live.exe --check-audio` e leia a última linha do log. Essa verificação dura dois segundos e informa se o microfone e o loopback da saída foram inicializados; ela não comprova ainda a qualidade de uma chamada real.

Se não houver som, execute `.\target\release\screen-voice-live.exe --check-output`: ele toca um tom curto na saída padrão sem usar a API. O log registra a cada dez segundos somente a duração de atividade de cada fonte, a quantidade de trechos transcritos e de blocos de áudio recebidos, sem guardar a conversa. O [desenho, pesquisa e roteiro de aceitação](docs/production-voice-agent.md) explicam o que foi implementado e o que falta validar.

Esta primeira versão identifica **fontes de áudio**, não pessoas por biometria: microfone sugere candidato, saída do computador sugere entrevistador. Quando as vozes se sobrepõem, a identificação pode falhar. Para reduzir eco, o programa suspende a captura da saída durante a janela da resposta; nesse intervalo pode perder parte da fala do entrevistador. Faça primeiro um teste curto com uma chamada simulada e confira a atribuição e o áudio antes de depender das dicas. A sessão GPT Live é cobrada pelo tempo em que fica conectada, inclusive com a fala desligada. Verifique as regras da entrevista e avise os participantes quando a captação ou o envio de áudio exigir consentimento.
