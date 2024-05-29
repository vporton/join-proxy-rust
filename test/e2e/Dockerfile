FROM rust:1.78

WORKDIR /usr/src/myapp

ENV DFXVM_INIT_YES=true
RUN curl -fsSL https://internetcomputer.org/install.sh | sh
ENV PATH=$HOME/.local/share/dfx/bin:$PATH
RUN $HOME/.local/share/dfx/bin/dfxvm default 0.20.0

RUN apt-get update
#RUN apt-get install -y ca-certificates
RUN apt-get install -y openssl npm libunwind8

RUN npm i -g ic-mops

# RUN openssl req -x509 -out localhost.crt -keyout localhost.key \
#   -newkey rsa:2048 -nodes -sha256 \
#   -subj '/CN=localhost' -extensions EXT -config \
#    "[dn]\nCN=localhost\n[req]\ndistinguished_name = dn\n[EXT]\nsubjectAltName=DNS:localhost\nkeyUsage=digitalSignature\nextendedKeyUsage=serverAuth"
RUN openssl req -x509 -out localhost.crt -keyout localhost.key \
  -newkey rsa:2048 -nodes -sha256 \
  -subj '/CN=localhost'
RUN openssl x509 -in localhost.crt -out localhost.pem -outform PEM
RUN cp -a localhost.crt /usr/local/share/ca-certificates
RUN update-ca-certificates
RUN mkdir -p test/e2e/tmpl/
RUN cp localhost.pem localhost.key test/e2e/tmpl/

COPY motoko/ ./motoko/
COPY proxy/ ./proxy/
COPY test/ ./test/
COPY Cargo.lock .
COPY Cargo.toml .
COPY dfx.json .
COPY mops.toml .

# --debug to speed up compilation
RUN cargo build

# CMD ./target/debug/test-server & ./target/debug/joining-proxy & sleep 2; curl https://localhost:8443/
CMD sh -c "export PATH=$HOME/.local/share/dfx/bin:$PATH && ./target/debug/e2e"
