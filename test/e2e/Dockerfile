FROM rust:1.78

WORKDIR /usr/src/myapp

ENV DFXVM_INIT_YES=true
RUN curl -fsSL https://internetcomputer.org/install.sh | sh
# ENV PATH=$HOME/.local/share/dfx/bin:$PATH
RUN $HOME/.local/share/dfx/bin/dfxvm default 0.19.0

#RUN apt-get install -y ca-certificates
RUN apt-get install -y openssl

# RUN openssl req -x509 -out localhost.crt -keyout localhost.key \
#   -newkey rsa:2048 -nodes -sha256 \
#   -subj '/CN=localhost' -extensions EXT -config \
#    "[dn]\nCN=localhost\n[req]\ndistinguished_name = dn\n[EXT]\nsubjectAltName=DNS:localhost\nkeyUsage=digitalSignature\nextendedKeyUsage=serverAuth"
RUN openssl req -x509 -out localhost.crt -keyout localhost.key \
  -newkey rsa:2048 -nodes -sha256 \
  -subj '/CN=localhost'
RUN cp -a localhost.crt /usr/local/share/ca-certificates
RUN update-ca-certificates
   
COPY motoko/ ./motoko/
COPY proxy/ ./proxy/
COPY test/ ./test/
COPY Cargo.lock .
COPY Cargo.toml .
COPY dfx.json .
COPY mops.toml .

# --debug to speed up compilation
RUN cargo build

CMD ./target/debug/e2e
