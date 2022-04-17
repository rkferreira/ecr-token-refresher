FROM rust:1.60 as builder

RUN mkdir /build
WORKDIR ./build
COPY ./Cargo* .
ADD src ./src
RUN cargo build --release

FROM rust:1.60-slim

RUN mkdir /app
COPY --from=builder /build/target/release/ecr-token-refresher /app
