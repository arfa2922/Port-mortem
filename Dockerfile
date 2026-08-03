# Reproducible build and verification for semver-rs.
#
#   docker build -t semver-rs .
#   docker run --rm semver-rs                    # run the full suite
#   docker run --rm semver-rs bench              # run benchmarks
#   docker run --rm semver-rs differential       # fuzz against the original
#
# Node is present because the original is used as a live oracle: the
# differential fuzzer runs both implementations on the same input and
# compares. Without it the port could only be checked against fixtures.

FROM rust:1.75-slim-bookworm

# Node for the oracle, git to fetch the original, curl for the Node repo.
RUN apt-get update && apt-get install -y --no-install-recommends \
        git \
        curl \
        ca-certificates \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Dependencies first, so source edits do not invalidate the layer.
COPY Cargo.toml ./
RUN mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && cargo build --release 2>/dev/null || true \
    && rm -rf src

COPY . .

# Fetch the original, pin its suite, and export the fixtures. This also
# writes kickoff.hash, which CI compares against the committed copy.
RUN bash scripts/fetch_original.sh

RUN cargo build --release

# Fail the image build if any unsafe block appears.
RUN if grep -Prn "^\s*unsafe\s" src/; then \
        echo "FAIL: unsafe block found" >&2; exit 1; \
    else \
        echo "verified: 0 unsafe blocks"; \
    fi

COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["test"]
