# ── Image "tool" (scratch) ────────────────────────────────────────
# For COPY --from=ghcr.io/pytgaen/fimod:latest /fimod /usr/local/bin/
# Contains only the static binary at /fimod.
FROM scratch AS tool
ARG FIMOD_BIN=fimod
COPY ${FIMOD_BIN} /fimod

# ── Image "runtime" (Ubuntu minimal) ─────────────────────────────
# For standalone use: docker run ghcr.io/pytgaen/fimod-runtime ...
FROM ubuntu:24.04 AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=tool /fimod /usr/local/bin/fimod
RUN chmod +x /usr/local/bin/fimod
ENTRYPOINT ["fimod"]
