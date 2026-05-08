// ResonatorAudio.ts — v2.2
let _ctx: AudioContext | null = null;
let _lastClick = 0;

function getCtx(): AudioContext | null {
    if (!_ctx) {
        try {
            const AudioContextClass = (window.AudioContext || (window as any).webkitAudioContext);
            _ctx = new AudioContextClass();
        } catch (e: any) {
            console.warn('[ResonatorAudio] AudioContext unavailable:', e.message);
            return null;
        }
    }
    if (_ctx && _ctx.state === 'suspended') _ctx.resume().catch(() => {});
    return _ctx;
}

export function initAudio() {
    getCtx();
}

export function playResonance(ramUsage: string | number) {
    const ctx = getCtx();
    if (!ctx) return;

    const now = ctx.currentTime;
    const ramInt = typeof ramUsage === 'string' ? parseInt(ramUsage.replace('MB', '')) : ramUsage;
    
    const interval = Math.max(0.05, 1.0 - ((ramInt || 0) / 2000));
    if (now - _lastClick < interval) return;

    try {
        const osc  = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = 'sine';
        osc.frequency.setValueAtTime((ramInt || 0) > 1800 ? 880 : 440, now);
        gain.gain.setValueAtTime(0.1, now);
        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.1);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.start();
        osc.stop(now + 0.1);
        _lastClick = now;
    } catch (e: any) {
        console.warn('[ResonatorAudio] playback error:', e.message);
    }
}
