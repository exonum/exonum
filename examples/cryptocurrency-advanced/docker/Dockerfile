FROM ubuntu:16.04

ENV ROCKSDB_LIB_DIR=/usr/lib/x86_64-linux-gnu
ENV SNAPPY_LIB_DIR=/usr/lib/x86_64-linux-gnu

RUN apt-get update \
  && apt-get install -y curl git gcc cpp \
  libssl-dev pkg-config libsodium-dev libsnappy-dev librocksdb-dev

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable

RUN curl -sL https://deb.nodesource.com/setup_8.x | bash \
  && apt-get install -y nodejs

RUN mv /root/.cargo/bin/* /usr/bin

WORKDIR /usr/src
RUN git clone https://github.com/exonum/exonum.git \
  && cd exonum/examples/cryptocurrency-advanced/backend \
  && cargo update && cargo install
WORKDIR /usr/src/exonum/examples/cryptocurrency-advanced
COPY launch.sh .

ENTRYPOINT ["sh", "./launch.sh"]
