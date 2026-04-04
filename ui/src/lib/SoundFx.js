export default {
    click() {
        try {
            const audio = new AudioContext();
            const oscillator = audio.createOscillator();
            const gain = audio.createGain();
            
            oscillator.type = 'square';
            oscillator.frequency.setValueAtTime(1200, audio.currentTime);
            gain.gain.setValueAtTime(0.15, audio.currentTime);
            gain.gain.exponentialRampToValueAtTime(0.001, audio.currentTime + 0.08);
            
            oscillator.connect(gain);
            gain.connect(audio.destination);
            oscillator.start();
            oscillator.stop(audio.currentTime + 0.1);
        } catch(e) {}
    },

    success() {
        try {
            const audio = new AudioContext();
            const notes = [880, 1100, 1320];
            let time = audio.currentTime;
            
            notes.forEach((freq, i) => {
                const osc = audio.createOscillator();
                const gain = audio.createGain();
                osc.type = 'sine';
                osc.frequency.value = freq;
                gain.gain.value = 0.2;
                gain.gain.exponentialRampToValueAtTime(0.001, time + 0.4 + i * 0.05);
                
                osc.connect(gain);
                gain.connect(audio.destination);
                osc.start(time + i * 0.08);
                osc.stop(time + 0.6);
            });
        } catch(e) {}
    },

    error() {
        try {
            const audio = new AudioContext();
            const osc = audio.createOscillator();
            const gain = audio.createGain();
            osc.type = 'sawtooth';
            osc.frequency.value = 180;
            gain.gain.value = 0.3;
            gain.gain.exponentialRampToValueAtTime(0.001, audio.currentTime + 0.6);
            
            osc.connect(gain);
            gain.connect(audio.destination);
            osc.start();
            osc.stop(audio.currentTime + 0.6);
        } catch(e) {}
    }
};
