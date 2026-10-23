# Práticas pesquisadas para o Screen Voice

Consulta ao acervo público do AI Engineer em 21/10/2026. As palestras cobrem agentes de voz conversacionais; este protótipo é acionado por tecla e não usa microfone, então detecção de fim de fala e interrupções não se aplicam ainda.

## Aplicadas

1. **Preparar o texto antes da síntese.** O texto enviado à voz agora remove marcação simples e palavras idênticas repetidas em sequência; a instrução de análise evita listas, Markdown e excesso de palavras. Uma palestra recomenda normalizar a saída do modelo antes do TTS para impedir que marcação e texto mal formatado sejam lidos em voz alta. [5 Voice Agent Failure Modes You'll Hit in Week One — Venky B, Plivo — 22:38](https://www.youtube.com/watch?v=vblnYHzBgS4&t=1358)
2. **Observar cada etapa sem registrar conteúdo da tela.** O log passa a marcar acionamento, captura, interpretação, início da geração de voz e reprodução concluída. Uma avaliação de voz precisa distinguir conteúdo textual, áudio e sequência de eventos; um texto correto pode esconder silêncio, tom ruim ou falha na reprodução. [The Transcript Looked Fine. The Call Wasn't. — Debugging Voice Agents, Arize — 2:12](https://www.youtube.com/watch?v=42Bz0TUfeOQ&t=132)
3. **Poucas capacidades com gatilhos claros.** Curamos três instruções curtas em `skills/`: orientar pela tela, pesquisar engenharia de IA e falar com naturalidade. O agente só recebe as ferramentas públicas `search` e `fetch` do AI Engineer, e a instrução define quando usá-las. A recomendação de começar com poucas ferramentas e avaliar o comportamento aparece em [Building Effective Voice Agents — 11:52](https://www.youtube.com/watch?v=-OXiljTJxQU&t=712). O uso de metadados e instruções de skills para ensinar tarefas específicas aparece em [Skills are the New SDKs — 14:06](https://www.youtube.com/watch?v=LC3-P7v3yoI&t=846).
4. **Ajustar a expressividade e avaliar pelo ouvido.** A instrução da voz explicita tom, ritmo e entonação; o modelo de fala foi fixado na versão `2025-12-15`. A palestra da OpenAI recomenda experimentar instruções de expressividade e avaliar tom e cadência no áudio, além do texto. [Building Effective Voice Agents — 10:14](https://www.youtube.com/watch?v=-OXiljTJxQU&t=614) e [Building Effective Voice Agents — 13:25](https://www.youtube.com/watch?v=-OXiljTJxQU&t=805).

## Próximo teste manual

Faça três acionamentos separados, aguardando cada fala terminar. Confira no log se cada tentativa chegou a `Reprodução concluída.` ou em qual etapa houve erro. Para avaliar o estilo, ouça se a resposta traz um próximo passo útil, se há palavras repetidas e se o tom é natural. Não coloque capturas ou transcrições privadas no relatório sem decisão explícita do usuário.

## Limites atuais

- A limpeza remove apenas repetições adjacentes óbvias no texto. Se a repetição for produzida pelo próprio áudio, é necessário ouvi-la e comparar com a resposta textual em uma amostra de teste autorizada.
- Erros `429 credit_balance_exhausted` vêm da API. O programa mostra e registra o erro; não repete automaticamente uma chamada que pode estar sem crédito disponível para a chave em uso.
- As skills são instruções curtas incorporadas ao executável na compilação. Editá-las exige recompilar.

## Pesquisa: duas pessoas na chamada e um agente no fone

O uso em entrevista por videochamada tem três participantes na experiência, mas só duas pessoas devem participar da chamada. O agente acompanha a conversa e oferece ajuda apenas ao usuário pelo fone. Esse padrão de *overlay* é apresentado em [The Voice-First AI Overlay: Designing Conversational Co-Pilots — 2:25](https://www.youtube.com/watch?v=y9YQc9a3gNw&t=145). A mesma palestra destaca que uma dica cedo demais interrompe, tarde demais perde utilidade e fora de contexto vira ruído: [The Voice-First AI Overlay: Designing Conversational Co-Pilots — 8:35](https://www.youtube.com/watch?v=y9YQc9a3gNw&t=515).

**Desenho proposto:** capturar separadamente o microfone do usuário e o áudio recebido da videochamada, com rótulos de origem e horários. Não depender de uma única mistura para adivinhar quem falou. O som do agente deve ir apenas ao fone, ficar fora do áudio enviado à chamada e fora da entrada que ele próprio analisa. A aplicação controla quando reproduzir uma dica; o agente fica em silêncio por padrão e fala quando solicitado. Se houver duas pessoas remotas no mesmo áudio ou falas sobrepostas, pode ser necessária identificação adicional de locutores; a diarização costuma produzir `speaker 1/2`, não nomes, e ainda erra em sobreposições e turnos curtos. [Beyond Transcription: Building Voice AI That Actually Understands Conversations — 10:41](https://www.youtube.com/watch?v=mFLlVpnGpds&t=641) e [Beyond Transcription: Building Voice AI That Actually Understands Conversations — 13:54](https://www.youtube.com/watch?v=mFLlVpnGpds&t=834).

O GPT Live pode manter interação de voz contínua e lidar com interrupções, mas a aplicação ainda precisa controlar a reprodução local e a política de quando o agente fala. Mutar a saída e parar de enviar áudio são operações distintas. Fontes: [Voice agents](https://developers.openai.com/api/docs/guides/voice-agents), [Managing GPT-Live sessions](https://developers.openai.com/api/docs/guides/live-conversations).

**Teste necessário antes de uma entrevista real:** uma chamada simulada com duas pessoas e o agente, incluindo perguntas interrompidas, silêncio, vozes sobrepostas e reprodução de uma dica no fone. Verificar separadamente a atribuição de fala, o tempo até a dica, se a dica chega no momento certo e se o áudio do agente vaza para a chamada. Erros na transcrição podem se propagar para sugestões erradas: [The Voice-First AI Overlay: Designing Conversational Co-Pilots — 12:47](https://www.youtube.com/watch?v=y9YQc9a3gNw&t=767).
