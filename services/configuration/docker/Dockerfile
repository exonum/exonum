FROM rust:1.23.0

WORKDIR /usr/src
RUN git clone https://github.com/exonum/exonum \
  && cd exonum/services/configuration \
  && cargo install --example configuration \
  && cd /usr/src \
  && rm -rf exonum
WORKDIR /exonum/configuration
COPY launch.sh .

ENTRYPOINT ["./launch.sh"]
