    const res = await fetch('/api/memories/archive', { method: 'POST' });
    if (res.ok) {
        neuralResponse = ">> LOCAL ARCHIVE CREATED IN PROJECT ROOT.";
    } else {
        neuralResponse = ">> ERROR: SERVER FAILED TO WRITE ARCHIVE.";
    }
}
