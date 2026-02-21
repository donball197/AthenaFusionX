FROM kalilinux/kali-rolling
RUN apt-get update && apt-get install -y \
    radare2 \
    checksec \
    binutils \
    python3 \
    && rm -rf /var/lib/apt/lists/*
CMD ["sleep", "infinity"]
