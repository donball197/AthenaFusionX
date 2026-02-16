let audioCtx;
let lastClick = 0;

export function initAudio() {
    if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
}

export function playResonance(ramUsage) {
    if (!audioCtx) return;
    
    const now = audioCtx.currentTime;
    const ramInt = parseInt(ramUsage.replace('MB', '')) || 0;
    
    // Calculate interval based on RAM usage (150MB = 1s, 1800MB = 0.05s)
    const interval = Math.max(0.05, 1.0 - (ramInt / 2000));

    if (now - lastClick > interval) {
        const osc = audioCtx.createOscillator();
        const gain = audioCtx.createGain();

        osc.type = 'sine';
        osc.frequency.setValueAtTime(ramInt > 1800 ? 880 : 440, now);
        
        gain.gain.setValueAtTime(0.1, now);
        gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.1);

        osc.connect(gain);
        gain.connect(audioCtx.destination);

        osc.start();
        osc.stop(now + 0.1);
        lastClick = now;
    }
}
