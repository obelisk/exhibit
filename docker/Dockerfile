FROM messense/rust-musl-cross:aarch64-musl as builder

RUN rustup component add rustfmt
RUN mkdir /tmp/exhibit
WORKDIR /tmp/exhibit
COPY . .

RUN cargo build --release


FROM alpine:3.6 as alpine
RUN apk add -U --no-cache ca-certificates

FROM scratch as runtime
COPY --from=alpine /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /tmp/exhibit/target/aarch64-unknown-linux-musl/release/exhibit /exhibit
USER 1000
ENTRYPOINT [ "/exhibit" ]
