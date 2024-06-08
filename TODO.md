TODO:

- Specify proxy's identity.

- Don't repeatedly ask for confirmation for the same hash.

- Heavy `Vec` copy operations may hinder performance.

- Make responses streaming (impossible due to caching?)

- Implement file-persistency for in-memory DB. Also, save on `SIG{INT,TERM}`.

- Redis storage support.

- Incrementing nonce to avoid upstream request replay attack.

- If the proxy is directed to its own URL, will this work as a DoS attack?

- Improve tests:

    - Use Docker `ENTRYPOINT` to call cargo tests.
    
    - Output which test is running now.
