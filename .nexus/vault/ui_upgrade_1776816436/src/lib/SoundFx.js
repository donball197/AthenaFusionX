// NEXUS Synthetic Audio Engine
const playTone = (freq, type, duration, volume) => {
    try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = type;
        osc.frequency.setValueAtTime(freq, ctx.currentTime);
        gain.gain.setValueAtTime(volume, ctx.currentTime);
        gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + duration);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.start();
        osc.stop(ctx.currentTime + duration);
    } catch(e) {}
};

export default {
    click: () => playTone(800, 'sine', 0.1, 0.1),
    processing: () => playTone(440, 'square', 0.05, 0.05),
    success: () => {
        playTone(1000, 'sine', 0.2, 0.1);
        setTimeout(() => playTone(1200, 'sine', 0.3, 0.1), 100);
    },
    error: () => playTone(150, 'sawtooth', 0.5, 0.2)
};
