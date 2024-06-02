FROM rust:1.78

WORKDIR /usr/src/myapp

ENV DFXVM_INIT_YES=true
RUN curl -fsSL https://internetcomputer.org/install.sh | sh
ENV PATH=$HOME/.local/share/dfx/bin:$PATH
RUN $HOME/.local/share/dfx/bin/dfxvm default 0.20.0

RUN apt-get update
RUN apt-get install -y openssl npm libunwind8

RUN npm i -g ic-mops

COPY test/e2e/localhost.ext .

# RUN openssl req -x509 -out localhost.crt -keyout localhost.key \
#   -newkey rsa:2048 -nodes -sha256 \
#   -subj '/CN=localhost'
# RUN openssl x509 -in localhost.crt -out localhost.pem -outform PEM
# RUN cp -a localhost.crt /usr/local/share/ca-certificates
# RUN update-ca-certificates
# RUN mkdir -p test/e2e/tmpl/
# RUN cp localhost.pem localhost.key test/e2e/tmpl/
RUN openssl genrsa -passout pass: -out CA.key -des3 2048
RUN openssl req -passin pass: -x509 -sha256 -new -nodes -key CA.key -out CA.pem -subj '/CN=ip6-localhost/O=My Company Name LTD./C=US'
RUN openssl genrsa -passout pass: -out localhost.key -des3 2048
RUN openssl req -passin pass: -new -key localhost.key -out localhost.csr -subj '/CN=ip6-localhost/O=My Company Name LTD./C=US'
RUN openssl x509 -passin pass: -req -in localhost.csr \
                  -CA CA.pem -CAkey CA.key \
                  -CAcreateserial -sha256 \
                  -extfile localhost.ext -out localhost.crt
RUN openssl rsa -passin pass: -in localhost.key -out localhost.decrypted.key
RUN mkdir -p test/e2e/tmpl/
RUN cp localhost.crt localhost.decrypted.key test/e2e/tmpl/
RUN cp localhost.crt /usr/local/share/ca-certificates
RUN update-ca-certificates

COPY motoko/ ./motoko/
COPY proxy/ ./proxy/
COPY test/ ./test/
COPY Cargo.lock .
COPY Cargo.toml .
COPY dfx.json .
COPY mops.toml .

# FIXME: This is for testing:
# RUN git clone https://github.com/vporton/ic.git
# WORKDIR /usr/src/myapp/ic
# RUN git checkout env_logger
# RUN apt-get install -y protobuf-compiler libunwind-dev llvm liblmdb-dev librocksdb-dev
# RUN cargo build -p ic-replica
# RUN cp target/debug/replica $HOME/.local/share/dfx/bin
# WORKDIR /usr/src/myapp

# --debug to speed up compilation
RUN cargo build

# RUN sh -c "echo ::1 localhost >> /etc/hosts"

CMD sh -c "export PATH=$HOME/.local/share/dfx/bin:$PATH && ./target/debug/e2e"
