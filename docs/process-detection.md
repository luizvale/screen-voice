# Teste de detecção de processos

## Escopo

Avaliar, em ambiente autorizado, se o monitor consegue identificar o processo do agente mesmo quando ele não apresenta janela. O teste não altera a lista de processos do Windows nem os sensores do monitor.

## Hipótese

Um monitor que acompanha somente janelas ou a categoria **Apps** pode deixar de exibir um programa sem janela. Isso não implica que o processo esteja oculto do sistema: a guia **Detalhes**, a auditoria de criação de processos e outros sensores podem registrá-lo.

## Evidência a coletar

| Fonte | Campos mínimos | Pré-condição |
| --- | --- | --- |
| Gerenciador de Tarefas, guia Detalhes | nome, PID, usuário, horário observado | acesso à sessão Windows |
| `Get-Process` | nome, PID, caminho, horário de início | consulta enquanto o processo estiver em execução |
| Evento de Segurança 4688 | novo PID, executável, processo criador, horário | Audit Process Creation habilitado |
| Evento de Segurança 4689 | PID encerrado, horário | Audit Process Termination habilitado |
| Sysmon, evento 1 | ProcessGuid, PID, imagem, linha de comando, processo pai, hash, horário | Sysmon instalado e regra de ProcessCreate ativa |
| Sysmon, evento 5 | ProcessGuid, PID, horário de término | Sysmon instalado e regra de ProcessTerminate ativa |

Não coloque chaves de API em argumentos de linha de comando nem nas capturas de evidência. Use uma amostra que não faça chamadas externas para testar apenas a detecção do processo.

## Procedimento

1. Registrar versão do Windows, nome e configuração do monitor, sensores habilitados e relógio de referência.
2. Iniciar a amostra com janela visível e anotar o PID e o horário.
3. Repetir com a janela oculta, mantendo o mesmo executável e a mesma conta.
4. Consultar as fontes acima em cada execução. Conferir se o PID e o horário correspondem à amostra; no Sysmon, usar também ProcessGuid para distinguir PIDs reutilizados.
5. Registrar separadamente ausência de coleta, falha de apresentação na interface e falha de alerta. Não chamar uma dessas situações de invisibilidade total.

Consulta rápida durante a execução:

```powershell
Get-Process -Name screen-voice | Select-Object Id, ProcessName, Path, StartTime, MainWindowTitle
```

Não filtre por `MainWindowTitle`: o campo vazio é esperado para um processo sem janela. Se a consulta pontual não encontrar o PID, verifique primeiro se o processo ainda está ativo. Logs ausentes podem significar auditoria desligada, filtro, retenção insuficiente ou falta de acesso. A linha de comando no evento 4688 exige uma política de auditoria adicional.

## Prova local em 21/10/2026

- Uma amostra `powershell.exe` iniciada com `Start-Process -WindowStyle Hidden` permaneceu ativa e apareceu em `Get-Process` (PID 34752), com `MainWindowTitle` vazio. Isso comprova apenas a distinção entre janela oculta e processo visível nessa sessão.
- A tentativa com `screen-voice.exe` terminou com código 1 antes da consulta: `não foi possível registrar CTRL+ALT+F24; ela pode estar em uso`. Assim, **não há resultado de detecção para esse executável** nesta prova. Repetir em sessão interativa com uma combinação de teclas disponível.
- O produto de monitoramento da competição ainda não foi testado; não há vulnerabilidade confirmada nele.

## Rodada cega com QA

1. O operador registra previamente o executável, PID, horário de início, conta de usuário e resultado de `Get-Process`. Guarda esse registro fora do material entregue ao QA.
2. Executa o agente numa sessão Windows interativa com uma tecla de atalho disponível. Para avaliar a hipótese de janela oculta, pode iniciar a amostra sem janela visível; não alterar nome, caminho, assinatura, eventos ou sensores durante a rodada.
3. Informa ao QA apenas a janela de tempo do teste e o conjunto de máquinas autorizadas. O QA procura o processo pelos meios disponíveis no monitor e registra consultas, alertas e evidências.
4. Se não encontrar, entrega uma pista por vez: (a) máquina e conta; (b) intervalo de início de cinco minutos; (c) nome do executável. Registrar em qual etapa a descoberta ocorreu.
5. Comparar o registro do operador com a coleta bruta, a interface e os alertas. Classificar o resultado como **coletado e encontrado**, **coletado mas não apresentado**, **apresentado mas não alertado**, **sem coleta comprovada** ou **teste inconclusivo**.

O critério de vulnerabilidade é uma falha reproduzível do monitor dentro do escopo e da configuração declarados. Dificuldade do QA em encontrar um processo sem pista, por si só, não comprova falha técnica.

## Registro de achado

- **Hipótese:**
- **Sensor e configuração:**
- **Passos reproduzíveis:**
- **Resultado observado (com horário e PID):**
- **Resultado esperado:**
- **Evidência:**
- **Impacto e limite da conclusão:**
- **Correção proposta:**

## Fontes

- [Gerenciador de Tarefas: a guia Detalhes lista processos de apps e de segundo plano](https://learn.microsoft.com/en-us/troubleshoot/windows-server/support-tools/support-tools-task-manager)
- [Windows: auditoria de criação de processos e evento 4688](https://learn.microsoft.com/en-us/windows/security/threat-protection/auditing/audit-process-creation)
- [Sysmon: evento 1, ProcessCreate](https://learn.microsoft.com/en-us/sysinternals/downloads/sysmon)
- [Get-Process no PowerShell](https://learn.microsoft.com/en-us/powershell/module/Microsoft.PowerShell.Management/get-process?view=powershell-5.1)
- [Auditoria de encerramento de processo, evento 4689](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-4689)
- [Sysmon: eventos 1 e 5](https://learn.microsoft.com/en-us/windows/security/operating-system-security/sysmon/sysmon-events)
- [Política para incluir a linha de comando no evento 4688](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/component-updates/command-line-process-auditing)
