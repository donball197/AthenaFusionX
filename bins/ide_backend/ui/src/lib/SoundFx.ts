// SoundFx.ts — Singleton Context Fix
let _ctx: AudioContext | null = null;

function getCtx(): AudioContext | null {
    if (!_ctx) {
        try {
            const AudioContextClass = (window.AudioContext || (window as any).webkitAudioContext);
            _ctx = new AudioContextClass();
        } catch (e) {
            console.warn('[SFX] AudioContext unavailable');
            return null;
        }
    }
    if (_ctx && _ctx.state === 'suspended') _ctx.resume().catch(() => {});
    return _ctx;
}

const playTone = (freq: number, type: OscillatorType, duration: number, volume: number) => {
    const ctx = getCtx();
    if (!ctx) return;
    try {
        const osc  = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = type;
        osc.frequency.setValueAtTime(freq, ctx.currentTime);
        gain.gain.setValueAtTime(volume, ctx.currentTime);
        gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + duration);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.start();
        osc.stop(ctx.currentTime + duration);
    } catch (e) {
        console.warn('[SFX] Tone playback failed');
    }
};

export default {
    click:      () => playTone(800,  'sine',     0.10, 0.10),
    processing: () => playTone(440,  'square',   0.05, 0.05),
    success: () => {
        playTone(1000, 'sine', 0.20, 0.10);
        setTimeout(() => playTone(1200, 'sine', 0.30, 0.10), 100);
    },
    error:      () => playTone(150,  'sawtooth', 0.50, 0.20),
};
