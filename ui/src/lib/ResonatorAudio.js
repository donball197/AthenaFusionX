// ResonatorAudio.js — v2.1 Singleton AudioContext
// FIX: Previous version created a new AudioContext on every initAudio()
//      call, exhausting the browser's ~6-instance cap within minutes.
//      Now uses the same lazy-singleton pattern as SoundFx.js.
let _ctx = null;
let _lastClick = 0;

function getCtx() {
    if (!_ctx) {
        try {
            _ctx = new (window.AudioContext || window.webkitAudioContext)();
        } catch (e) {
            console.warn('[ResonatorAudio] AudioContext unavailable:', e.message);
            return null;
        }
    }
    if (_ctx.state === 'suspended') _ctx.resume().catch(() => {});
    return _ctx;
}

// initAudio: call once on first user gesture to unlock autoplay policy
export function initAudio() {
    getCtx(); // just forces creation + resume
}

export function playResonance(ramUsage) {
    const ctx = getCtx();
    if (!ctx) return;

    const now = ctx.currentTime;
    const ramInt = parseInt(String(ramUsage).replace('MB', '')) || 0;

    // Interval: 150MB=1s, 1800MB=0.05s
    const interval = Math.max(0.05, 1.0 - (ramInt / 2000));
    if (now - _lastClick < interval) return;

    try {
        const osc  = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = 'sine';
        osc.frequency.setValueAtTime(ramInt > 1800 ? 880 : 440, now);
        gain.gain.setValueAtTime(0.1, now);
        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.1);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.start();
        osc.stop(now + 0.1);
        _lastClick = now;
    } catch (e) {
        console.warn('[ResonatorAudio] playback error:', e.message);
    }
}
