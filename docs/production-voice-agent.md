# Treinador de entrevista por voz: implementação e critérios de produção

Atualizado em 23/10/2026. Uso previsto: **simulação consentida com um amigo**, em videochamada no Windows, com dicas apenas no fone do candidato. Esta versão é um protótipo operacional; os critérios abaixo distinguem o que já foi implementado do que precisa ser comprovado antes de depender dela numa sessão longa.

## Pesquisa aplicada

- **Turnos e interrupções pertencem ao pipeline de áudio.** VAD, decisão de fim de turno e limpeza da fila quando alguém interrompe mudam a experiência mesmo com o mesmo modelo. Aplicação: reprodução local espera 500 ms de silêncio e a fila é cortada quando há nova fala detectada. [Designing Voice Agents for Real Conversations — 3:54](https://www.youtube.com/watch?v=hMlLw1LeIK8&t=234)
- **Medir a resposta que foi ouvida, além da geração do modelo.** Latência de transcrição, rede, raciocínio e início do áudio têm causas diferentes. Aplicação: log de etapas, início de reprodução e contagens de áudio sem guardar o conteúdo. [Engineering voice agents: Latency, quality, and scale — 10:42](https://www.youtube.com/watch?v=N7b1PJc7SFc&t=642)
- **Avaliar cenários, não só prompts isolados.** Mudanças podem corrigir uma entrevista e piorar outra; é preciso repetir casos com resultado esperado. Aplicação: roteiro de aceitação abaixo, com sucesso e falha separados. [Voice Agents: the good, the bad, and the ugly — 13:43](https://www.youtube.com/watch?v=pzmbleiOfCM&t=823)
- **Falas sobrepostas reduzem a qualidade da atribuição.** A origem microfone/loopback é uma estimativa de canal, não diarização por pessoa. Aplicação: o prompt não afirma quem falou quando há sobreposição. [Beyond Transcription: Building Voice AI That Actually Understands Conversations — 17:19](https://www.youtube.com/watch?v=mFLlVpnGpds&t=1039)

A [documentação oficial da OpenAI sobre agentes de voz](https://developers.openai.com/api/docs/guides/voice-agents) recomenda comparar conclusão da tarefa, tempo até uma resposta audível, interrupções, silêncio indevido e falhas de sessão. [Managing GPT-Live sessions](https://developers.openai.com/api/docs/guides/live-conversations) esclarece que `session.commentary.appended` confirma a inserção de contexto, **não** que uma dica foi falada ou ouvida; o WebSocket também não fornece um evento autoritativo de fim do áudio. Por isso a janela de reprodução usa uma aproximação local e exige teste de escuta.

## Fluxo implementado

```text
microfone ────────┐
                  ├─ áudio misturado 24 kHz ─ GPT Live (contexto contínuo)
áudio da chamada ┘                     │
                                      └─ áudio da resposta ─ fila local ─ fone

Ctrl+Alt+H: próxima pista curta      Ctrl+Alt+A: resposta de estudo
Ctrl+Alt+E: captura da tela → Responses visão → contexto factual → primeira pista
Ctrl+Alt+L: corta a fala atual        Ctrl+Alt+Shift+L: encerra a sessão
```

O agente começa calado. Os atalhos de pista e resposta abrem uma janela local de reprodução e enviam um pedido ao GPT Live. O áudio só começa após uma pausa de cerca de 500 ms nos canais humanos. Se o microfone ou a chamada ficarem ativos durante a dica, a janela fecha e a fila é descartada. Uma janela sem áudio expira após 30 s; após uma fala e 3 s sem novos blocos de áudio, fecha. Um pedido seguinte substitui a fila anterior.

`Ctrl+Alt+E` captura **somente o monitor principal** e chama a API Responses uma vez para extrair fatos visíveis do exercício. O resultado é enviado como contexto ao GPT Live; só após a confirmação `session.thinking.appended` é pedida a primeira pista. Uma captura em andamento não bloqueia o envio contínuo de áudio; `Ctrl+Alt+L` cancela a intenção de reproduzir o resultado. A tela e a conversa não são gravadas em disco; as duas são enviadas à OpenAI enquanto as respectivas funções são usadas. Evite capturar chaves ou dados de terceiros: o prompt não os oculta da imagem enviada. O modelo visual pode não ler código pequeno, cortado ou fora da tela. O MCP do AI Engineer continua disponível apenas no programa de captura de tela separado; o Live não o consulta.

## Critérios para chamar de pronto para uso confiável

1. **Áudio e roteamento:** em uma chamada simulada, confirmar que o amigo ouve apenas o candidato; o agente sai apenas no fone e a captura da chamada não contém eco perceptível. Testar fones, dispositivos padrão alterados e reconexão da chamada.
2. **Turnos:** repetir perguntas com pausas, hesitação, interrupção, sobreposição e ruído. Verificar que a dica não começa enquanto alguém fala e para quando o candidato interrompe. A interrupção pelo amigo durante a própria voz do agente ainda **não é garantida**: o loopback é suspenso nessa hora para evitar realimentação. Para resolver isso, separar fisicamente/virtualmente a captura da chamada da saída do agente ou adicionar cancelamento de eco validado.
3. **Conteúdo:** dez exercícios representativos de código e system design, com enunciados legíveis e não legíveis. A primeira pista não deve entregar a solução; a segunda deve avançar; a resposta completa deve explicar estratégia e limites sem inventar código ou experiências.
4. **Entrega e latência:** registrar por tentativa o tempo do atalho até o primeiro áudio realmente reproduzido, a proporção de pedidos que produziram dica útil e a proporção de dicas cortadas. Avaliar mediana e P95 por tipo de pedido, incluindo screenshot.
5. **Falhas:** testar API 401/429, rede caída, WebSocket fechado, troca de dispositivo, atalho ocupado e saída sem som. O processo deve informar a falha sem registrar chave, captura, transcrição ou áudio. A reconexão automática ainda não foi implementada; hoje a sessão termina e avisa pelo diálogo/log.
6. **Privacidade e custo:** pedir ao amigo que concorde com o envio do áudio à API antes da simulação. Confirmar que `Ctrl+Alt+Shift+L` recebe `session.closed`; a sessão custa enquanto permanece conectada, mesmo sem resposta falada. Definir limites de orçamento na conta OpenAI antes de sessões longas.

## Teste manual rápido

1. Configure a chave da API, fone como saída padrão e microfone físico na chamada. Inicie o Live e confirme `session.started` no log.
2. Faça o amigo ler uma pergunta. Pressione `Ctrl+Alt+H` enquanto ele termina; a dica deve esperar a pausa e conter uma ideia concreta. Pressione `Ctrl+Alt+L` durante a fala e confirme que ela para.
3. Mostre um exercício legível no monitor principal; pressione `Ctrl+Alt+E`. Peça depois outra pista com `Ctrl+Alt+H` e a estratégia completa com `Ctrl+Alt+A`.
4. Interrompa a dica pelo microfone, teste uma pergunta com falas sobrepostas e verifique o que seu amigo ouviu. Confira somente os eventos e contagens em `%LOCALAPPDATA%\screen-voice\screen-voice-live.log`.
5. Encerre com `Ctrl+Alt+Shift+L` e confirme `session.closed`. Não use uma sessão real como teste de aceitação inicial.

Não existem testes automatizados que substituam a chamada simulada e a escuta humana. A captura de tela, a geração de voz e o comportamento do GPT Live dependem de dispositivos, rede e conta de API reais.
