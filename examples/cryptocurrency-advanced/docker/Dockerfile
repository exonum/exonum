FROM ubuntu:18.04

ENV ROCKSDB_LIB_DIR=/usr/lib
ENV SNAPPY_LIB_DIR=/usr/lib/x86_64-linux-gnu

RUN apt-get update \
    && apt-get install -y software-properties-common \
    && add-apt-repository ppa:maarten-fonville/protobuf \
    && apt-get update \
    && apt-get install -y curl git \
    build-essential libsodium-dev libsnappy-dev \
    librocksdb-dev pkg-config clang-7 lldb-7 lld-7 \
    libprotobuf-dev protobuf-compiler \
    python3-pip python3-setuptools

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain=stable

RUN curl -sL https://deb.nodesource.com/setup_8.x | bash \
  && apt-get install -y nodejs

RUN pip3 install exonum-launcher --upgrade --no-binary=protobuf

WORKDIR /usr/src
RUN git clone --branch v1.0.0-rc.1 https://github.com/exonum/exonum.git \
  && mv /root/.cargo/bin/* /usr/bin \
  && cd exonum/examples/cryptocurrency-advanced/backend \
  && cargo update && cargo install --path .
  # skip frontend part until JS light client is fixed
  # && cd ../frontend && npm install && npm run build
WORKDIR /usr/src/exonum/examples/cryptocurrency-advanced
COPY launch.sh .
COPY cryptocurrency_advanced.yaml .

ENTRYPOINT ["./launch.sh"]
