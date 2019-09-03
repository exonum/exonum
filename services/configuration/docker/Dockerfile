FROM ubuntu:18.04

ENV ROCKSDB_LIB_DIR=/usr/lib
ENV SNAPPY_LIB_DIR=/usr/lib/x86_64-linux-gnu

RUN apt-get update \
    && apt-get install -y software-properties-common \
    && add-apt-repository ppa:maarten-fonville/protobuf \ 
    && apt-get update \
    && apt-get install -y curl git \
    build-essential libsodium-dev libsnappy-dev \
    libssl-dev librocksdb-dev pkg-config clang-7 lldb-7 lld-7 \ 
    libprotobuf-dev protobuf-compiler

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain=stable

WORKDIR /usr/src
RUN git clone --branch v0.12 https://github.com/exonum/exonum.git \
  && mv /root/.cargo/bin/* /usr/bin \
  && cd exonum/services/configuration \
  && cargo update && cargo install --example configuration --path .
WORKDIR /usr/src/services/configuration
COPY launch.sh .

ENTRYPOINT ["./launch.sh"]
