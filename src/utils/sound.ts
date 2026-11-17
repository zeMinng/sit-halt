/** 提醒提示音：用 Web Audio 合成，避免引入音频资源文件。 */

export type SoundWave = "sine" | "square" | "triangle" | "sawtooth";

export const SOUND_WAVE_OPTIONS: { value: SoundWave; label: string }[] = [
  { value: "sine", label: "正弦 · 柔和" },
  { value: "triangle", label: "三角 · 圆润" },
  { value: "square", label: "方波 · 明亮" },
  { value: "sawtooth", label: "锯齿 · 尖锐" },
];

const VALID_WAVES: SoundWave[] = ["sine", "square", "triangle", "sawtooth"];

function normalizeWave(wave: unknown): SoundWave {
  return VALID_WAVES.includes(wave as SoundWave) ? (wave as SoundWave) : "sine";
}

let context: AudioContext | null = null;

function getContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return null;
  if (!context) context = new Ctor();
  return context;
}

/** 三声短促提示音，音量 0~1，音色由 wave 指定。 */
export function playReminderSound(volume = 0.7, wave: unknown = "sine"): void {
  const ctx = getContext();
  if (!ctx) return;
  if (ctx.state === "suspended") {
    void ctx.resume();
  }
  const gain = ctx.createGain();
  gain.gain.value = Math.min(1, Math.max(0.05, volume));
  gain.connect(ctx.destination);

  const type = normalizeWave(wave);
  const notes = [0, 0.28, 0.56];
  notes.forEach((offset, index) => {
    const oscillator = ctx.createOscillator();
    oscillator.type = type;
    oscillator.frequency.value = index % 2 === 0 ? 880 : 660;
    oscillator.connect(gain);
    const start = ctx.currentTime + offset;
    oscillator.start(start);
    oscillator.stop(start + 0.18);
  });
}
