#![cfg(windows)]
#![windows_subsystem = "windows"]

use std::{collections::VecDeque, env, error::Error, ffi::c_void, fs::{self, OpenOptions}, io::Write, sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, AtomicU64, Ordering}}, time::{Duration, SystemTime, UNIX_EPOCH}};

use base64::{engine::general_purpose::STANDARD, Engine};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::{client::IntoClientRequest, http::HeaderValue, Message as WsMessage}};

const RATE: usize = 24_000;
const FRAME: usize = 480;
const MOD_ALT: u32 = 1;
const MOD_CONTROL: u32 = 2;
const MOD_SHIFT: u32 = 4;
const MOD_NOREPEAT: u32 = 0x4000;
const WM_HOTKEY: u32 = 0x0312;

#[repr(C)]
struct Point { x: i32, y: i32 }
#[repr(C)]
struct WinMessage {
    hwnd: *mut c_void, message: u32, w_param: usize, l_param: isize,
    time: u32, pt: Point, l_private: u32,
}
#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hwnd: *mut c_void, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: *mut c_void, id: i32) -> i32;
    fn GetMessageW(message: *mut WinMessage, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

fn log_event(message: &str) {
    static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOG_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let Some(base) = env::var_os("LOCALAPPDATA") else { return };
    let dir = std::path::PathBuf::from(base).join("screen-voice");
    if fs::create_dir_all(&dir).is_err() { return }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(dir.join("screen-voice-live.log")) {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let _ = writeln!(file, "{stamp} {message}");
    }
}

fn log_capture_error(error: &str) {
    static LAST_ERROR: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let previous = LAST_ERROR.load(Ordering::Relaxed);
    if now.saturating_sub(previous) >= 5 && LAST_ERROR.compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        log_event(&format!("erro de captura: {error}"));
    }
}

fn alert(message: &str) {
    let content: Vec<u16> = format!("{message}\n\nDetalhes em %LOCALAPPDATA%\\screen-voice\\screen-voice-live.log\0").encode_utf16().collect();
    let title: Vec<u16> = "Screen Voice Live\0".encode_utf16().collect();
    unsafe { MessageBoxW(std::ptr::null_mut(), content.as_ptr(), title.as_ptr(), 0x10); }
}

fn spawn_hotkeys(tx: mpsc::UnboundedSender<bool>) -> Result<(), Box<dyn Error>> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || unsafe {
        // Ctrl+Alt+L alterna a fala; Ctrl+Alt+Shift+L encerra a sessão.
        if RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'L' as u32) == 0 {
            log_event("falha ao registrar Ctrl+Alt+L");
            let _ = ready_tx.send(Err("Ctrl+Alt+L já está em uso"));
            return;
        }
        if RegisterHotKey(std::ptr::null_mut(), 2, MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, 'L' as u32) == 0 {
            log_event("falha ao registrar atalho de saída");
            UnregisterHotKey(std::ptr::null_mut(), 1);
            let _ = ready_tx.send(Err("não foi possível registrar Ctrl+Alt+Shift+L"));
            return;
        }
        let _ = ready_tx.send(Ok(()));
        let mut event = std::mem::zeroed::<WinMessage>();
        while GetMessageW(&mut event, std::ptr::null_mut(), 0, 0) > 0 {
            if event.message == WM_HOTKEY && tx.send(event.w_param != 2).is_err() { break }
            if event.message == WM_HOTKEY && event.w_param == 2 { break }
        }
        UnregisterHotKey(std::ptr::null_mut(), 1);
        UnregisterHotKey(std::ptr::null_mut(), 2);
    });
    ready_rx.recv_timeout(Duration::from_secs(3))??;
    Ok(())
}

type Queue = Arc<Mutex<VecDeque<f32>>>;

fn push_mono(queue: &Queue, data: &[f32], channels: usize, input_rate: usize, phase: &mut usize) {
    let mut samples = queue.lock().unwrap();
    for frame in data.chunks_exact(channels) {
        let mono = frame.iter().sum::<f32>() / channels as f32;
        *phase += RATE;
        while *phase >= input_rate {
            *phase -= input_rate;
            samples.push_back(mono.clamp(-1.0, 1.0));
        }
    }
    // shortcut: discard stale audio after a stall; a production pipeline should report packet loss.
    while samples.len() > RATE * 2 { samples.pop_front(); }
}

fn capture_stream(device: &cpal::Device, queue: Queue, feedback: Option<Arc<AtomicBool>>) -> Result<cpal::Stream, Box<dyn Error>> {
    // CPAL ativa loopback ao abrir uma saída como entrada, mas a configuração
    // desse dispositivo deve ser obtida pela API de saída.
    let supported = device.default_input_config().or_else(|_| device.default_output_config())?;
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels as usize;
    let rate = config.sample_rate as usize;
    let name = device.description().map(|x| x.to_string()).unwrap_or_else(|_| "dispositivo".into());
    log_event(&format!("captura {name}: {rate} Hz, {channels} canais"));
    let on_error = |error: cpal::Error| log_capture_error(&error.to_string());
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut phase = 0;
            device.build_input_stream(config.clone(), move |data: &[f32], _| {
                if feedback.as_ref().is_some_and(|guard| guard.load(Ordering::Relaxed)) { queue.lock().unwrap().clear(); return }
                push_mono(&queue, data, channels, rate, &mut phase);
            }, on_error, None)?
        }
        cpal::SampleFormat::I16 => {
            let mut phase = 0;
            device.build_input_stream(config.clone(), move |data: &[i16], _| {
                if feedback.as_ref().is_some_and(|guard| guard.load(Ordering::Relaxed)) { queue.lock().unwrap().clear(); return }
                let samples: Vec<f32> = data.iter().map(|x| *x as f32 / 32768.0).collect();
                push_mono(&queue, &samples, channels, rate, &mut phase);
            }, on_error, None)?
        }
        cpal::SampleFormat::U16 => {
            let mut phase = 0;
            device.build_input_stream(config.clone(), move |data: &[u16], _| {
                if feedback.as_ref().is_some_and(|guard| guard.load(Ordering::Relaxed)) { queue.lock().unwrap().clear(); return }
                let samples: Vec<f32> = data.iter().map(|x| (*x as f32 - 32768.0) / 32768.0).collect();
                push_mono(&queue, &samples, channels, rate, &mut phase);
            }, on_error, None)?
        }
        format => return Err(format!("formato de captura não suportado: {format:?}").into()),
    };
    stream.play()?;
    Ok(stream)
}

fn fill_output<T: Copy>(out: &mut [T], channels: usize, rate: usize, queue: &Queue, enabled: &AtomicBool, feedback: &AtomicBool, phase: &mut usize, current: &mut f32, started: &mut bool, convert: impl Fn(f32) -> T) {
    let mut samples = queue.lock().unwrap();
    for frame in out.chunks_exact_mut(channels) {
        if !enabled.load(Ordering::Relaxed) {
            samples.clear();
            *current = 0.0;
            *started = false;
            feedback.store(false, Ordering::Relaxed);
        } else {
            if !*started && samples.len() >= RATE / 20 { *started = true; }
            if *started {
                *phase += RATE;
                while *phase >= rate {
                    *phase -= rate;
                    *current = samples.pop_front().unwrap_or(0.0);
                    if samples.is_empty() { *started = false; }
                }
            } else {
                *current = 0.0;
            }
            feedback.store(!samples.is_empty() || current.abs() > 0.001, Ordering::Relaxed);
        }
        for sample in frame { *sample = convert(*current); }
    }
}

fn playback_stream(device: &cpal::Device, queue: Queue, enabled: Arc<AtomicBool>, feedback: Arc<AtomicBool>) -> Result<cpal::Stream, Box<dyn Error>> {
    let supported = device.default_output_config()?;
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels as usize;
    let rate = config.sample_rate as usize;
    log_event(&format!("saída: {} Hz, {} canais", rate, channels));
    let on_error = |error| log_event(&format!("erro de reprodução: {error}"));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let (mut phase, mut current, mut started) = (0, 0.0, false);
            device.build_output_stream(config.clone(), move |out: &mut [f32], _| fill_output(out, channels, rate, &queue, &enabled, &feedback, &mut phase, &mut current, &mut started, |x| x), on_error, None)?
        }
        cpal::SampleFormat::I16 => {
            let (mut phase, mut current, mut started) = (0, 0.0, false);
            device.build_output_stream(config.clone(), move |out: &mut [i16], _| fill_output(out, channels, rate, &queue, &enabled, &feedback, &mut phase, &mut current, &mut started, |x| (x * 32767.0) as i16), on_error, None)?
        }
        cpal::SampleFormat::U16 => {
            let (mut phase, mut current, mut started) = (0, 0.0, false);
            device.build_output_stream(config.clone(), move |out: &mut [u16], _| fill_output(out, channels, rate, &queue, &enabled, &feedback, &mut phase, &mut current, &mut started, |x| ((x + 1.0) * 32767.5) as u16), on_error, None)?
        }
        format => return Err(format!("formato de saída não suportado: {format:?}").into()),
    };
    stream.play()?;
    Ok(stream)
}

fn take_frame(queue: &Queue) -> Vec<f32> {
    let mut samples = queue.lock().unwrap();
    (0..FRAME).map(|_| samples.pop_front().unwrap_or(0.0)).collect()
}

fn energy(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

fn mix_frame(mic: &[f32], call: &[f32]) -> Vec<u8> {
    mic.iter().zip(call).flat_map(|(a, b)| {
        let mixed = (a * 0.7 + b * 0.7).clamp(-1.0, 1.0);
        ((mixed * 32767.0) as i16).to_le_bytes()
    }).collect()
}

async fn send_json<S>(ws: &mut S, event: Value) -> Result<(), Box<dyn Error>>
where S: futures_util::Sink<WsMessage> + Unpin,
      S::Error: Error + 'static {
    ws.send(WsMessage::Text(event.to_string().into())).await?;
    Ok(())
}

async fn run() -> Result<(), Box<dyn Error>> {
    if env::args().any(|arg| arg == "--check-output") {
        let device = cpal::default_host().default_output_device().ok_or("saída padrão não encontrada")?;
        let queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
        for i in 0..RATE / 2 {
            queue.lock().unwrap().push_back((i as f32 * std::f32::consts::TAU * 440.0 / RATE as f32).sin() * 0.12);
        }
        let enabled = Arc::new(AtomicBool::new(true));
        let feedback = Arc::new(AtomicBool::new(false));
        let _speaker = playback_stream(&device, queue, enabled, feedback)?;
        std::thread::sleep(Duration::from_secs(1));
        log_event("teste local de saída concluído");
        return Ok(());
    }
    if env::args().any(|arg| arg == "--check-audio") {
        let host = cpal::default_host();
        let mic = host.default_input_device().ok_or("microfone padrão não encontrado")?;
        let call = host.default_output_device().ok_or("saída padrão não encontrada")?;
        let mic_queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
        let call_queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
        let _mic = capture_stream(&mic, mic_queue.clone(), None)?;
        let _call = capture_stream(&call, call_queue.clone(), None)?;
        std::thread::sleep(Duration::from_secs(2));
        log_event(&format!("verificação local de áudio: microfone {} amostras; chamada {} amostras", mic_queue.lock().unwrap().len(), call_queue.lock().unwrap().len()));
        return Ok(());
    }
    let api_key = env::var("OPENAI_API_KEY").map_err(|_| "defina OPENAI_API_KEY antes de iniciar")?;
    if api_key.trim().is_empty() { return Err("OPENAI_API_KEY está vazia".into()) }

    let host = cpal::default_host();
    let mic_device = host.default_input_device().ok_or("microfone padrão não encontrado")?;
    let call_device = host.default_output_device().ok_or("saída padrão não encontrada")?;
    let mic_queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
    let call_queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
    let play_queue: Queue = Arc::new(Mutex::new(VecDeque::new()));
    let enabled = Arc::new(AtomicBool::new(false));
    let feedback = Arc::new(AtomicBool::new(false));
    let _mic = capture_stream(&mic_device, mic_queue.clone(), None)?;
    // WASAPI/CPAL: usar a saída como entrada ativa o loopback do dispositivo.
    let _call = capture_stream(&call_device, call_queue.clone(), Some(feedback.clone()))?;
    let _speaker = playback_stream(&call_device, play_queue.clone(), enabled.clone(), feedback)?;
    let (hot_tx, mut hot_rx) = mpsc::unbounded_channel();
    spawn_hotkeys(hot_tx)?;

    let mut request = "wss://api.openai.com/v1/live/sessions".into_client_request()?;
    request.headers_mut().insert("Authorization", HeaderValue::from_str(&format!("Bearer {api_key}"))?);
    let (mut ws, _) = tokio::time::timeout(Duration::from_secs(15), connect_async(request)).await??;
    send_json(&mut ws, json!({
        "type":"session.start", "event_id":"event_start", "session": {
            "model":"gpt-live-1",
            "instructions":"Você é um copiloto privado de entrevista em português brasileiro. Há três participantes: o candidato no microfone, o entrevistador no áudio do computador e você, ouvido apenas pelo candidato. Ouça continuamente e guarde o contexto. Com a fala desativada, permaneça em silêncio. Com a fala ativada, responda a pedidos diretos do candidato e, após uma pergunta do entrevistador, ofereça ao candidato uma sugestão concreta de resposta. Fale em uma a três frases completas; comece pela sugestão útil, sem aberturas como 'essa é clássica' ou apenas 'tá'. Se o candidato claramente interromper para falar com você, pare e escute. Ruídos e falas de fundo não são pedidos diretos. Política de confirmações: evite interjeições vazias; toda fala deve trazer uma orientação. Se houver sobreposição, não afirme com certeza quem falou. Nunca aja como o candidato nem responda ao entrevistador. O áudio humano é misturado, com avisos estimados de fonte dominante que podem atrasar ou falhar. Política de delegação: gere <|SpawnThinking|> quando uma pergunta técnica ou comportamental exigir análise ou uma resposta estruturada. Aguarde a análise antes de concluir a dica. Não delegue apenas para cumprimentar ou confirmar que está ouvindo.",
            "audio":{"format":{"type":"audio/pcm","rate":24000},"output":{"voice":"bossa"}},
            "delegation":{"type":"responses","responses":{"model":"gpt-6-luna","instructions":"Você apoia um candidato em entrevista de emprego. Use o contexto da conversa para produzir uma sugestão prática de resposta em português brasileiro, com até três frases. Comece pelo conteúdo útil. Se faltar informação essencial, diga qual é. Não invente experiências do candidato.","max_output_tokens":240}}
        }
    })).await?;
    log_event("conectado ao GPT Live; aguardando session.started; fala desligada");

    let mut ready = false;
    let mut closing_at: Option<std::time::Instant> = None;
    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut source = "indefinida";
    let mut candidate_source = "indefinida";
    let mut source_ticks = 0;
    let mut event_id = 0u64;
    let mut mic_active_frames = 0u32;
    let mut call_active_frames = 0u32;
    let mut output_chunks = 0u32;
    let mut input_transcript_parts = 0u32;
    let mut output_transcript_chars = 0usize;
    let mut last_report = std::time::Instant::now();
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if closing_at.is_some_and(|since| since.elapsed() > Duration::from_secs(15)) {
                    return Err("a sessão não confirmou o encerramento em 15 segundos".into());
                }
                if closing_at.is_some() { continue }
                if !ready { continue }
                let mic = take_frame(&mic_queue);
                let call = take_frame(&call_queue);
                let mic_level = energy(&mic);
                let call_level = energy(&call);
                if mic_level > 0.015 { mic_active_frames += 1; }
                if call_level > 0.015 { call_active_frames += 1; }
                if last_report.elapsed() >= Duration::from_secs(10) {
                    log_event(&format!("atividade 10s: microfone={}ms chamada={}ms transcrição_entrada={} partes transcrição_saida={} caracteres áudio_saida={} blocos", mic_active_frames * 20, call_active_frames * 20, input_transcript_parts, output_transcript_chars, output_chunks));
                    mic_active_frames = 0;
                    call_active_frames = 0;
                    input_transcript_parts = 0;
                    output_chunks = 0;
                    output_transcript_chars = 0;
                    last_report = std::time::Instant::now();
                }
                let detected = if mic_level > 0.015 && mic_level > call_level * 1.8 { "candidato" }
                    else if call_level > 0.015 && call_level > mic_level * 1.8 { "entrevistador" }
                    else { "indefinida" };
                if detected == candidate_source { source_ticks += 1; } else { candidate_source = detected; source_ticks = 1; }
                if source_ticks == 12 && detected != source && detected != "indefinida" {
                    source = detected;
                    event_id += 1;
                    send_json(&mut ws, json!({"type":"session.thinking.append","event_id":format!("speaker_{event_id}"),"delegation_id":null,"content":format!("Fonte de áudio dominante neste momento: {source}. Essa é uma estimativa baseada nos canais; se ambos falarem, não atribua com certeza.")})).await?;
                }
                send_json(&mut ws, json!({"type":"session.input_audio.append","audio":STANDARD.encode(mix_frame(&mic, &call))})).await?;
            }
            Some(toggle) = hot_rx.recv() => {
                if !toggle {
                    enabled.store(false, Ordering::SeqCst);
                    play_queue.lock().unwrap().clear();
                    if ready {
                        send_json(&mut ws, json!({"type":"session.close"})).await?;
                        closing_at = Some(std::time::Instant::now());
                    } else { break }
                    log_event("encerramento solicitado pelo atalho");
                    continue;
                }
                let now_enabled = !enabled.load(Ordering::SeqCst);
                enabled.store(now_enabled, Ordering::SeqCst);
                if !now_enabled { play_queue.lock().unwrap().clear(); }
                log_event(if now_enabled { "fala ativada" } else { "fala desativada; escuta continua" });
                if ready {
                    event_id += 1;
                    send_json(&mut ws, json!({"type":"session.instructions.append","event_id":format!("mode_{event_id}"),"delegation_id":null,"content":if now_enabled {"A fala do copiloto está ativada. Responda a pedidos diretos do candidato e, após pergunta do entrevistador, dê uma sugestão concreta em até três frases completas. Vá direto à sugestão, sem interjeições."} else {"A fala do copiloto está desativada. Continue ouvindo e formando contexto, mas não produza resposta falada até nova instrução."}})).await?;
                    if now_enabled {
                        event_id += 1;
                        send_json(&mut ws, json!({"type":"session.commentary.append","event_id":format!("hint_{event_id}"),"delegation_id":null,"content":"Se houve uma pergunta recente do entrevistador, dê agora uma sugestão concreta de resposta em uma ou duas frases completas. Caso não tenha havido, diga apenas 'Pronto, estou ouvindo'."})).await?;
                    }
                }
            }
            packet = ws.next() => {
                let Some(packet) = packet else { return Err("conexão GPT Live encerrada".into()) };
                let packet = packet?;
                if let WsMessage::Text(text) = packet {
                    let value: Value = serde_json::from_str(&text)?;
                    match value["type"].as_str().unwrap_or("") {
                        "session.started" => {
                            mic_queue.lock().unwrap().clear();
                            call_queue.lock().unwrap().clear();
                            ready = true;
                            log_event("session.started; áudio contínuo iniciado");
                            if enabled.load(Ordering::SeqCst) {
                                event_id += 1;
                                send_json(&mut ws, json!({"type":"session.instructions.append","event_id":format!("mode_{event_id}"),"delegation_id":null,"content":"A fala do copiloto está ativada. Se houver pergunta recente do entrevistador, dê uma dica curta ao candidato; caso contrário, escute."})).await?;
                            }
                        }
                        "session.output_audio.delta" if enabled.load(Ordering::SeqCst) => {
                            output_chunks += 1;
                            if let Some(delta) = value["delta"].as_str() {
                                let bytes = STANDARD.decode(delta)?;
                                let mut samples = play_queue.lock().unwrap();
                                for pair in bytes.chunks_exact(2) { samples.push_back(i16::from_le_bytes([pair[0],pair[1]]) as f32 / 32768.0); }
                                while samples.len() > RATE * 8 { samples.pop_front(); }
                            }
                        }
                        "session.input_transcript.delta" => { input_transcript_parts += 1; }
                        "session.delegation.created" => { log_event("análise de resposta iniciada"); }
                        "response.event" if value.pointer("/event/type").and_then(Value::as_str) == Some("response.completed") => {
                            log_event("análise de resposta concluída");
                        }
                        "session.output_transcript.delta" => {
                            output_transcript_chars += value["delta"].as_str().map(str::len).unwrap_or(0);
                        }
                        "session.instructions.appended" | "session.commentary.appended" => {
                            log_event(value["type"].as_str().unwrap_or("contexto aceito"));
                        }
                        "error" | "session.error" => {
                            let code = value.pointer("/error/code").and_then(Value::as_str).unwrap_or("desconhecido");
                            log_event(&format!("erro GPT Live: {code}"));
                            return Err(format!("GPT Live retornou erro: {code}").into());
                        }
                        "session.closed" => { log_event("session.closed"); break; }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        log_event(&format!("falha: {error}"));
        alert(&error.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mix_preserves_frame_size_and_limits_pcm() {
        let bytes = mix_frame(&vec![1.0; FRAME], &vec![1.0; FRAME]);
        assert_eq!(bytes.len(), FRAME * 2);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 32767);
    }
    #[test]
    fn downsample_48k_to_24k() {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let mut phase = 0;
        push_mono(&queue, &vec![0.5; 960], 1, 48_000, &mut phase);
        assert_eq!(queue.lock().unwrap().len(), FRAME);
    }
    #[test]
    fn upsample_16k_to_24k() {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let mut phase = 0;
        push_mono(&queue, &vec![0.5; 320], 1, 16_000, &mut phase);
        assert_eq!(queue.lock().unwrap().len(), FRAME);
    }
}
