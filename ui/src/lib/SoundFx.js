// NEXUS Synthetic Audio Engine — v2.0 Singleton Context
// FIX: Single AudioContext reused across all sound events.
// Browsers hard-cap AudioContext instances at ~6; creating one per click
// exhausts the budget in seconds and causes silent failures.

let _ctx = null;

function getCtx() {
    if (!_ctx) {
        try {
            _ctx = new (window.AudioContext || window.webkitAudioContext)();
        } catch (e) {
            console.warn('[SFX] AudioContext unavailable:', e.message);
            return null;
        }
    }
    // Resume context if suspended (browser autoplay policy)
    if (_ctx.state === 'suspended') {
        _ctx.resume().catch(() => {});
    }
    return _ctx;
}

const playTone = (freq, type, duration, volume) => {
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
        console.warn('[SFX] Tone playback failed:', e.message);
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
